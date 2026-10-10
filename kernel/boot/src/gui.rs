//! The Asolaria desktop: wallpaper, desktop icons, movable windows, a taskbar with a start
//! menu and clock, mouse/touchpad input and a complete keyboard path.
//!
//! Runs on UEFI boot services (GOP framebuffer, ConIn keyboard, Simple/Absolute Pointer). The
//! scene is composed into a back buffer clipped to the dirty rectangle, then only that rectangle
//! is presented; the cursor is redrawn on the framebuffer after each present. Integer only.
//! E=0: the desktop starts nothing; every action is a click or a keystroke by the operator.

use crate::apps::{self, App, Buf, Term, View, APPS};
use crate::bridge::{Bridge, Msg, State as BridgeState};
use crate::console::{self, Action, Ctx, Sink, Tone};
use crate::gfx::{mix, rgb, text_w, Canvas, Rect, Screen, CURSOR_H, CURSOR_W};
use crate::pointer::Pointers;
use crate::rtc::{self, BootTime};
use crate::uefi::{BootServices, Bs, Guid};
use core::ffi::c_void;

const GOP_GUID: Guid = Guid {
    d1: 0x9042_a9de,
    d2: 0x23dc,
    d3: 0x4a38,
    d4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[repr(C)]
struct ModeInfo {
    version: u32,
    w: u32,
    h: u32,
    fmt: u32,
    masks: [u32; 4],
    pps: u32,
}
#[repr(C)]
struct Mode {
    max_mode: u32,
    mode: u32,
    info: *mut ModeInfo,
    size_of_info: usize,
    fb: u64,
    fb_size: usize,
}
#[repr(C)]
struct Gop {
    query_mode: unsafe extern "efiapi" fn(*mut Gop, u32, *mut usize, *mut *mut ModeInfo) -> usize,
    set_mode: unsafe extern "efiapi" fn(*mut Gop, u32) -> usize,
    _blt: usize,
    mode: *mut Mode,
}

const TB: i32 = 44; // taskbar height
const TITLE: i32 = 32; // window title-bar height
const NAPP: usize = 8;
const MENU_N: usize = 11; // 8 apps + text console + restart + power off
const COLS: usize = 96;
const ROWS: usize = 400;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    None,
    Desktop,
    Icon(usize),
    Start,
    Task(usize),
    Tray,
    Menu(usize),
    Title(usize),
    Close(usize),
    Min(usize),
    Body(usize),
}

#[derive(Clone, Copy)]
struct Win {
    r: Rect,
    open: bool,
    min: bool,
    scroll: i32,
}

struct Desk<'a, 'b> {
    c: &'a mut Ctx<'b>,
    scr: Screen,
    cv: Canvas,
    wall: Option<Canvas>,
    ptr: Pointers,
    mx: i32,
    my: i32,
    drawn: (i32, i32),
    wins: [Win; NAPP],
    z: [usize; NAPP],
    nz: usize,
    desk_focus: bool,
    sel: usize,
    menu: bool,
    menu_sel: usize,
    kptr: bool,
    kheld: bool,
    btn: bool,
    drag: Option<(usize, i32, i32)>,
    last_click: (u64, usize),
    hover: Hit,
    term: Term,
    /// The Claude terminal (Opus 5.5 through the host bridge on COM3).
    cterm: Term,
    bridge: Bridge,
    claude_busy: bool,
    last_cmd: [u8; 72],
    last_len: usize,
    tpms: u64,
    t0: u64,
    now: BootTime,
    last_rtc: u64,
    caret: bool,
    last_blink: u64,
    confirm: Option<(u32, u64)>,
    mem: Option<(u64, u64)>,
    fw: [u8; 48],
    fw_len: usize,
    fw_rev: u32,
    dirty: Rect,
    arrow: (u16, u64, i32),
    motion_logged: bool,
}

/// Start the desktop. Returns only if there is no usable framebuffer (or memory for one).
pub(crate) unsafe fn run(c: &mut Ctx) {
    let bs = Bs((*c.st).boot_services as *mut BootServices);
    let gop = bs.locate_protocol(&GOP_GUID) as *mut Gop;
    if gop.is_null() || (*gop).mode.is_null() {
        crate::serial_print(b"  GUI|op=unavailable|why=no_gop|json=0\r\n");
        return;
    }
    pick_mode(bs, gop);
    let m = &*(*gop).mode;
    if m.info.is_null() || m.fb == 0 {
        crate::serial_print(b"  GUI|op=unavailable|why=no_framebuffer|json=0\r\n");
        return;
    }
    let info = &*m.info;
    // Usable formats: 0 RGBx, 1 BGRx, and 2 (bit masks) when the masks are byte-aligned 8-8-8
    // in one of those two orders. 3 (Blt only) has no linear framebuffer.
    let Some(fmt) = effective_fmt(info.fmt, &info.masks) else {
        crate::serial_print(b"  GUI|op=unavailable|why=pixel_format|json=0\r\n");
        return;
    };
    if info.w < 640 || info.h < 480 {
        crate::serial_print(b"  GUI|op=unavailable|why=size|json=0\r\n");
        return;
    }
    // The framebuffer must hold every row we will write: stride >= width and
    // size >= stride * height * 4 bytes.
    let need = info.pps as u64 * info.h as u64 * 4;
    if info.pps < info.w || (m.fb_size as u64) < need {
        crate::serial_print(b"  GUI|op=unavailable|why=framebuffer_geometry|json=0\r\n");
        return;
    }
    let (w, h) = (info.w as i32, info.h as i32);
    let Some(back) = bs.alloc_slice::<u32>((w * h) as usize, 0) else {
        crate::serial_print(b"  GUI|op=unavailable|why=no_memory|json=0\r\n");
        return;
    };
    let wall = bs
        .alloc_slice::<u32>((w * h) as usize, 0)
        .map(|px| Canvas::new(px, w, h));
    let (Some(cells), Some(tones)) = (
        bs.alloc_slice::<u8>(COLS * ROWS, b' '),
        bs.alloc_slice::<u8>(COLS * ROWS, 0),
    ) else {
        crate::serial_print(b"  GUI|op=unavailable|why=no_memory|json=0\r\n");
        return;
    };
    let (Some(ccells), Some(ctones)) = (
        bs.alloc_slice::<u8>(COLS * ROWS, b' '),
        bs.alloc_slice::<u8>(COLS * ROWS, 0),
    ) else {
        crate::serial_print(b"  GUI|op=unavailable|why=no_memory|json=0\r\n");
        return;
    };
    let scr = Screen {
        fb: m.fb as *mut u32,
        pps: info.pps as i32,
        w,
        h,
        fmt,
    };
    let ptr = Pointers::find(bs, (*c.st).console_in_handle);
    let mem = bs.memory_totals();
    // TSC rate against the firmware's own 20 ms stall (integer ticks per millisecond).
    let a = rtc::rdtsc();
    bs.stall(20_000);
    let tpms = ((rtc::rdtsc() - a) / 20).max(1);
    let mut fw = [0u8; 48];
    let mut fw_len = 0;
    let vend = (*c.st).firmware_vendor;
    if !vend.is_null() {
        while fw_len < fw.len() {
            let ch = *vend.add(fw_len);
            if ch == 0 {
                break;
            }
            fw[fw_len] = if (0x20..0x7F).contains(&ch) {
                ch as u8
            } else {
                b'?'
            };
            fw_len += 1;
        }
    }
    let fw_rev = (*c.st).firmware_revision;
    let (ns, na) = ptr.counts();
    c.gui_ok = true;
    c.pointer_simple = ns;
    c.pointer_abs = na;
    c.text_cursor(false);

    let mut d = Desk {
        c,
        scr,
        cv: Canvas::new(back, w, h),
        wall,
        ptr,
        mx: w / 2,
        my: h / 2,
        drawn: (w / 2, h / 2),
        wins: [Win {
            r: Rect::EMPTY,
            open: false,
            min: false,
            scroll: 0,
        }; NAPP],
        z: [0; NAPP],
        nz: 0,
        desk_focus: true,
        sel: 0,
        menu: false,
        menu_sel: 0,
        kptr: false,
        kheld: false,
        btn: false,
        drag: None,
        last_click: (0, usize::MAX),
        hover: Hit::None,
        term: Term::new(cells, tones, COLS),
        cterm: Term::new(ccells, ctones, COLS),
        // SAFETY: COM3 port I/O, only under a hypervisor (see bridge.rs).
        bridge: unsafe { Bridge::open() },
        claude_busy: false,
        last_cmd: [0; 72],
        last_len: 0,
        tpms,
        t0: rtc::rdtsc(),
        now: rtc::now(),
        last_rtc: 0,
        caret: true,
        last_blink: 0,
        confirm: None,
        mem,
        fw,
        fw_len,
        fw_rev,
        dirty: Rect::EMPTY,
        arrow: (0, 0, 0),
        motion_logged: false,
    };
    d.log_ready();
    if let Some(wl) = d.wall.as_mut() {
        draw_wallpaper(wl);
    }
    d.welcome();
    d.claude_welcome();
    d.open(App::Console);
    // The Claude terminal always starts, on top and focused.
    d.open(App::Claude);
    d.dirty = d.cv.bounds();
    d.event_loop();
}

/// Map a GOP pixel format to the two layouts `Screen` writes (0 = RGBx, 1 = BGRx).
/// PixelBitMask (2) qualifies only when its masks are exactly one of those byte layouts.
fn effective_fmt(fmt: u32, masks: &[u32; 4]) -> Option<u32> {
    match fmt {
        0 | 1 => Some(fmt),
        2 => match (masks[0], masks[1], masks[2]) {
            (0x0000_00FF, 0x0000_FF00, 0x00FF_0000) => Some(0),
            (0x00FF_0000, 0x0000_FF00, 0x0000_00FF) => Some(1),
            _ => None,
        },
        _ => None,
    }
}

/// If the firmware left a small mode (< 1024 wide), switch to the largest mode that is at most
/// 1920x1200 so the desktop has room. Never touches a mode that is already >= 1024 wide.
unsafe fn pick_mode(bs: Bs, gop: *mut Gop) {
    let m = &*(*gop).mode;
    if m.info.is_null() || (*m.info).w >= 1024 {
        return;
    }
    let mut best: Option<(u32, u32)> = None; // (mode, area)
    for i in 0..m.max_mode.min(64) {
        let mut sz: usize = 0;
        let mut inf: *mut ModeInfo = core::ptr::null_mut();
        if ((*gop).query_mode)(gop, i, &mut sz, &mut inf) != 0 || inf.is_null() {
            continue;
        }
        let (w, h) = ((*inf).w, (*inf).h);
        let ok = effective_fmt((*inf).fmt, &(*inf).masks).is_some();
        bs.free(inf as *mut c_void);
        if ok && w <= 1920 && h <= 1200 && w >= 1024 {
            let area = w * h;
            if best.map_or(true, |(_, a)| area > a) {
                best = Some((i, area));
            }
        }
    }
    if let Some((i, _)) = best {
        let st = ((*gop).set_mode)(gop, i);
        crate::serial_print(if st == 0 {
            b"  GUI|op=set_mode|status=ok|json=0\r\n"
        } else {
            b"  GUI|op=set_mode|status=refused|json=0\r\n"
        });
    }
}

/// Wallpaper, rendered once: deep-navy gradient, two soft glows, a dot grid, the 3x3x3 cube and
/// the name.
fn draw_wallpaper(cv: &mut Canvas) {
    let (w, h) = (cv.w, cv.h);
    let (gx, gy) = ((w * 74 / 100) as i64, (h * 30 / 100) as i64);
    let r1 = (w.max(h) * 60 / 100) as i64;
    let (vx, vy) = ((w * 8 / 100) as i64, (h * 95 / 100) as i64);
    let r2 = (w.max(h) * 45 / 100) as i64;
    let glow1 = rgb(0x17, 0x4e, 0x6c);
    let glow2 = rgb(0x34, 0x1d, 0x5e);
    for y in 0..h {
        let base = apps::BG_TOP;
        let row = mix(base, apps::BG_BOT, (y as u32 * 256) / (h as u32 - 1).max(1));
        for x in 0..w {
            let (xi, yi) = (x as i64, y as i64);
            let d1 = (xi - gx) * (xi - gx) + (yi - gy) * (yi - gy);
            let d2 = (xi - vx) * (xi - vx) + (yi - vy) * (yi - vy);
            let mut c = row;
            if d1 < r1 * r1 {
                c = mix(c, glow1, ((r1 * r1 - d1) * 150 / (r1 * r1)) as u32);
            }
            if d2 < r2 * r2 {
                c = mix(c, glow2, ((r2 * r2 - d2) * 110 / (r2 * r2)) as u32);
            }
            if x % 32 == 16 && y % 32 == 16 {
                c = mix(c, apps::CYAN, 34);
            }
            cv.px[(y * w + x) as usize] = c;
        }
    }
    let (cx, cy, s) = (w * 72 / 100, h * 40 / 100, h * 19 / 100);
    apps::iso_cube(cv, cx, cy, s, 64, rgb(0x2c, 0x7a, 0x90));
    let ty = cy + s + 34;
    cv.text_center(
        (cx - 400, 800),
        ty,
        b"ASOLARIA ASI OS",
        rgb(0x9a, 0xf3, 0xea),
        3,
        true,
    );
    cv.text_center(
        (cx - 400, 800),
        ty + 58,
        b"balanced-ternary kernel  .  27 = 3^3  .  integer only  .  E=0",
        rgb(0x6f, 0x86, 0xa8),
        1,
        true,
    );
}

impl Desk<'_, '_> {
    fn now_ms(&self) -> u64 {
        (rtc::rdtsc().wrapping_sub(self.t0)) / self.tpms
    }

    fn log_ready(&self) {
        let mut b = Buf::new();
        let (ns, na) = self.ptr.counts();
        b.s(b"  GUI|op=ready|w=")
            .dec(self.scr.w as u64)
            .s(b"|h=")
            .dec(self.scr.h as u64);
        b.s(b"|fmt=")
            .dec(self.scr.fmt as u64)
            .s(b"|ptr_simple=")
            .dec(ns as u64);
        b.s(b"|ptr_abs=")
            .dec(na as u64)
            .s(b"|handles=")
            .dec(self.ptr.simple_handles as u64);
        b.s(b"/").dec(self.ptr.abs_handles as u64).s(b"|splitter=");
        b.dec(self.ptr.via_splitter as u64)
            .s(b"|phys=")
            .dec(self.ptr.phys as u64);
        b.s(b"|usb_connect=")
            .dec(self.ptr.connected_usb as u64)
            .s(b"|ps2=");
        b.s(match self.ptr.ps2_probe {
            None => b"not_needed",
            Some(crate::ps2mouse::Probe::Ready) => b"ready",
            Some(crate::ps2mouse::Probe::NoAck) => b"no_ack",
            Some(crate::ps2mouse::Probe::NoController) => b"no_controller",
        });
        b.s(b"|tsc_khz=")
            .dec(self.tpms)
            .s(b"|e=0|json=0")
            .raw(b"\r\n");
        // SAFETY: COM1 port I/O.
        unsafe { crate::serial_print(b.get()) };
        if let Some(r) = self.ptr.i2c_report {
            for k in 0..2 {
                let mut b = Buf::new();
                b.s(b"  GUI|op=i2c_hid|");
                if k == 0 {
                    crate::apps::i2c_line1(&mut b, &r);
                } else {
                    crate::apps::i2c_line2(&mut b, &r);
                }
                b.s(b"|json=0").raw(b"\r\n");
                // SAFETY: COM1 port I/O.
                unsafe { crate::serial_print(b.get()) };
            }
        }
    }

    fn ptr_note(&self) -> &'static [u8] {
        if self.ptr.i2c_active() {
            return b"driver: kernel I2C-HID touchpad";
        }
        if self.ptr.ps2_active() {
            return b"driver: kernel PS/2";
        }
        if self.ptr.phys > 0 {
            return b"driver: firmware";
        }
        if let Some(r) = self.ptr.i2c_report {
            if r.stage != crate::i2chid::Stage::Ready && r.controllers > 0 {
                return match r.stage {
                    crate::i2chid::Stage::BarUnassigned => b"I2C-HID: controller has no BAR",
                    crate::i2chid::Stage::NotDesignWare => b"I2C-HID: controller did not answer",
                    crate::i2chid::Stage::NoDevice => b"I2C-HID: no touchpad answered",
                    crate::i2chid::Stage::NoMouseCollection => {
                        b"I2C-HID: touchpad has no mouse mode"
                    }
                    _ => b"I2C-HID: not ready",
                };
            }
        }
        match self.ptr.ps2_probe {
            Some(crate::ps2mouse::Probe::NoAck) => b"PS/2: no mouse answered",
            Some(crate::ps2mouse::Probe::NoController) => b"PS/2: no controller",
            _ => b"",
        }
    }

    fn log(&self, op: &[u8], what: &[u8]) {
        let mut b = Buf::new();
        b.s(b"  GUI|op=")
            .s(op)
            .s(b"|")
            .s(what)
            .s(b"|json=0")
            .raw(b"\r\n");
        // SAFETY: COM1 port I/O.
        unsafe { crate::serial_print(b.get()) };
    }

    fn claude_welcome(&mut self) {
        let t = &mut self.cterm;
        t.tone(Tone::Accent);
        t.put(b"CLAUDE  .  Opus 5.5  .  seat ACER-CLAUDE-FABLE5\n");
        t.tone(Tone::Normal);
        t.put(b"Type a question and press Enter. It goes over COM3 to the host bridge, which\n");
        t.put(b"answers through the operator's own Claude Code harness (signed in, Asolaria\n");
        t.put(b"MCPs, FABLE5 office). No credential lives in this kernel.\n\n");
        t.tone(Tone::Warn);
        t.put(b"  bridge: ");
        t.put(self.bridge.state.text());
        t.put(b"\n\n");
        t.tone(Tone::Normal);
        let mut b = Buf::new();
        b.s(b"  GUI|op=claude_bridge|state=")
            .s(self.bridge.state.text())
            .s(b"|json=0")
            .raw(b"\r\n");
        // SAFETY: COM1 port I/O.
        unsafe { crate::serial_print(b.get()) };
    }

    fn claude_submit(&mut self) {
        let n = self.cterm.ilen;
        let mut q = [0u8; 72];
        q[..n].copy_from_slice(&self.cterm.input[..n]);
        self.cterm.ilen = 0;
        if n == 0 {
            return;
        }
        self.cterm.tone(Tone::Warn);
        self.cterm.put(b"opus> ");
        self.cterm.tone(Tone::Normal);
        self.cterm.put(&q[..n]);
        self.cterm.put(b"\n");
        if !self.bridge.live() {
            self.cterm.tone(Tone::Error);
            self.cterm.put(b"  ");
            self.cterm.put(self.bridge.state.text());
            self.cterm.put(b"\n");
            self.cterm.tone(Tone::Normal);
        } else if self.claude_busy {
            self.cterm.tone(Tone::Warn);
            self.cterm.put(b"  still answering the last question\n");
            self.cterm.tone(Tone::Normal);
        } else {
            // SAFETY: COM3 port I/O (bridge is live only under a hypervisor with a UART).
            if unsafe { self.bridge.ask(&q[..n]) } {
                self.claude_busy = true;
            } else {
                self.cterm.tone(Tone::Error);
                self.cterm
                    .put(b"  COM3 did not take the question (port stuck)\n");
                self.cterm.tone(Tone::Normal);
            }
        }
        let r = self.wins[App::Claude.idx()].r;
        self.mark(Self::win_area(r));
    }

    /// Move host-bridge lines into the Claude terminal (bounded per frame).
    fn claude_pump(&mut self) {
        let mut changed = false;
        for _ in 0..32 {
            // SAFETY: COM3 port I/O (no-op unless the bridge is live).
            let Some(m) = (unsafe { self.bridge.poll() }) else {
                break;
            };
            changed = true;
            match m {
                Msg::Hello(who) => {
                    // A (re)connected bridge has nothing in flight (opus55 F5).
                    self.claude_busy = false;
                    self.cterm.tone(Tone::Accent);
                    self.cterm.put(b"  connected: ");
                    self.cterm.put(who);
                    self.cterm.put(b"\n");
                    self.cterm.tone(Tone::Normal);
                }
                Msg::Out(s) => {
                    self.cterm.put(s);
                    self.cterm.put(b"\n");
                }
                Msg::End(s) => {
                    self.claude_busy = false;
                    self.cterm.tone(Tone::Accent);
                    self.cterm.put(b"  -- ");
                    self.cterm.put(s);
                    self.cterm.put(b"\n\n");
                    self.cterm.tone(Tone::Normal);
                }
                Msg::Busy(s) => {
                    self.claude_busy = false;
                    self.cterm.tone(Tone::Warn);
                    self.cterm.put(b"  busy: ");
                    self.cterm.put(s);
                    self.cterm.put(b"\n");
                    self.cterm.tone(Tone::Normal);
                }
                Msg::Other => {}
            }
        }
        if changed {
            let w = self.wins[App::Claude.idx()];
            if w.open && !w.min {
                self.mark(Self::win_area(w.r));
            }
        }
    }

    fn welcome(&mut self) {
        let t = &mut self.term;
        t.tone(Tone::Accent);
        t.put(b"ASOLARIA ASI OS  .  console  .  kernel ");
        t.put(asolaria_kernel_core::KERNEL_VERSION.as_bytes());
        t.put(b"\n");
        t.tone(Tone::Normal);
        t.put(b"The same envelope REPL as the text console: every command goes through init.\n");
        t.put(b"Type 'help'. F1 shows mouse and keyboard controls.\n\n");
        console::info(&mut self.term, self.c);
        self.term.put(b"\n");
    }

    // ---------------------------------------------------------------- geometry
    fn per_col(&self) -> i32 {
        ((self.scr.h - TB - 16) / 96).max(1)
    }

    fn icon_cell(&self, i: usize) -> Rect {
        let pc = self.per_col();
        let (col, row) = (i as i32 / pc, i as i32 % pc);
        Rect::new(14 + col * 104, 14 + row * 96, 100, 92)
    }

    fn start_rect(&self) -> Rect {
        Rect::new(8, self.scr.h - TB + 6, 132, TB - 12)
    }

    fn task_rect(&self, a: usize) -> Option<Rect> {
        let mut x = 150;
        for app in APPS {
            let i = app.idx();
            if !self.wins[i].open {
                continue;
            }
            if i == a {
                return Some(Rect::new(x, self.scr.h - TB + 6, 150, TB - 12));
            }
            x += 156;
        }
        None
    }

    fn tray_rect(&self) -> Rect {
        Rect::new(self.scr.w - 330, self.scr.h - TB, 330, TB)
    }

    fn menu_rect(&self) -> Rect {
        let mh = 66 + MENU_N as i32 * 30 + 14;
        Rect::new(8, self.scr.h - TB - 6 - mh, 300, mh)
    }

    fn menu_item(&self, i: usize) -> Rect {
        let m = self.menu_rect();
        let extra = if i >= NAPP { 10 } else { 0 };
        Rect::new(m.x + 6, m.y + 64 + i as i32 * 30 + extra, m.w - 12, 28)
    }

    fn close_rect(r: Rect) -> Rect {
        Rect::new(r.x + r.w - 34, r.y + 7, 24, 18)
    }

    fn min_rect(r: Rect) -> Rect {
        Rect::new(r.x + r.w - 64, r.y + 7, 24, 18)
    }

    fn body(r: Rect) -> Rect {
        Rect::new(r.x + 1, r.y + TITLE, r.w - 2, r.h - TITLE - 1)
    }

    /// Rect including the drop shadow (what must be repainted when a window changes).
    fn win_area(r: Rect) -> Rect {
        Rect::new(r.x - 2, r.y - 2, r.w + 10, r.h + 14)
    }

    fn focused(&self) -> Option<usize> {
        if self.desk_focus {
            return None;
        }
        (0..self.nz)
            .rev()
            .map(|k| self.z[k])
            .find(|&a| !self.wins[a].min)
    }

    fn hit(&self, x: i32, y: i32) -> Hit {
        if self.menu {
            let m = self.menu_rect();
            if m.contains(x, y) {
                return (0..MENU_N)
                    .find(|&i| self.menu_item(i).contains(x, y))
                    .map_or(Hit::None, Hit::Menu);
            }
        }
        for k in (0..self.nz).rev() {
            let a = self.z[k];
            let w = &self.wins[a];
            if w.min || !w.r.contains(x, y) {
                continue;
            }
            if Self::close_rect(w.r).contains(x, y) {
                return Hit::Close(a);
            }
            if Self::min_rect(w.r).contains(x, y) {
                return Hit::Min(a);
            }
            if y < w.r.y + TITLE {
                return Hit::Title(a);
            }
            return Hit::Body(a);
        }
        if y >= self.scr.h - TB {
            if self.start_rect().contains(x, y) {
                return Hit::Start;
            }
            for a in 0..NAPP {
                if self.task_rect(a).is_some_and(|r| r.contains(x, y)) {
                    return Hit::Task(a);
                }
            }
            return Hit::Tray;
        }
        for i in 0..NAPP {
            if self.icon_cell(i).contains(x, y) {
                return Hit::Icon(i);
            }
        }
        Hit::Desktop
    }

    fn hit_rect(&self, h: Hit) -> Rect {
        match h {
            Hit::Icon(i) => self.icon_cell(i),
            Hit::Start => self.start_rect(),
            Hit::Task(a) => self.task_rect(a).unwrap_or(Rect::EMPTY),
            Hit::Menu(i) => self.menu_item(i),
            Hit::Close(a) => Self::close_rect(self.wins[a].r),
            Hit::Min(a) => Self::min_rect(self.wins[a].r),
            _ => Rect::EMPTY,
        }
    }

    fn mark(&mut self, r: Rect) {
        self.dirty = self.dirty.union(&r);
    }

    fn mark_all(&mut self) {
        self.dirty = self.cv.bounds();
    }

    // ---------------------------------------------------------------- window management
    fn raise(&mut self, a: usize) {
        if let Some(p) = (0..self.nz).find(|&k| self.z[k] == a) {
            for k in p..self.nz - 1 {
                self.z[k] = self.z[k + 1];
            }
            self.z[self.nz - 1] = a;
        }
        self.wins[a].min = false;
        self.desk_focus = false;
        self.mark_all();
    }

    fn open(&mut self, app: App) {
        let a = app.idx();
        if !self.wins[a].open {
            let (dw, dh) = app.size();
            let (sw, sh) = (self.scr.w, self.scr.h - TB);
            let w = dw.min(sw - 24);
            let h = dh.min(sh - 24);
            // Cascade from just right of the icon column, leaving the wallpaper mark visible.
            let k = self.nz as i32;
            let x = (150 + k * 30).clamp(8, (sw - w - 8).max(8));
            let y = (36 + k * 28).clamp(8, (sh - h - 8).max(8));
            self.wins[a] = Win {
                r: Rect::new(x, y, w, h),
                open: true,
                min: false,
                scroll: 0,
            };
            self.z[self.nz] = a;
            self.nz += 1;
            self.log(b"open", app.name());
        }
        self.raise(a);
    }

    fn close(&mut self, a: usize) {
        if !self.wins[a].open {
            return;
        }
        self.wins[a].open = false;
        if let Some(p) = (0..self.nz).find(|&k| self.z[k] == a) {
            for k in p..self.nz - 1 {
                self.z[k] = self.z[k + 1];
            }
            self.nz -= 1;
        }
        if a == App::Power.idx() {
            self.confirm = None;
        }
        if self.drag.is_some_and(|d| d.0 == a) {
            self.drag = None;
        }
        self.log(b"close", APPS[a].name());
        self.mark_all();
    }

    fn minimize(&mut self, a: usize) {
        self.wins[a].min = true;
        if self.focused().is_none() {
            self.desk_focus = true;
        }
        self.mark_all();
    }

    /// F3 / Tab: cycle focus through the open windows, then the desktop.
    fn cycle(&mut self) {
        if self.nz == 0 {
            self.desk_focus = true;
            self.mark_all();
            return;
        }
        if self.desk_focus {
            let a = self.z[0];
            self.raise(a);
        } else if let Some(f) = self.focused() {
            // Send the focused window to the bottom; if it was the last one, go to the desktop.
            let bottom = self.z[0] == f;
            if let Some(p) = (0..self.nz).find(|&k| self.z[k] == f) {
                for k in (1..=p).rev() {
                    self.z[k] = self.z[k - 1];
                }
                self.z[0] = f;
            }
            if bottom || self.nz == 1 {
                self.desk_focus = true;
            }
            self.mark_all();
        }
    }

    // ---------------------------------------------------------------- actions
    fn menu_activate(&mut self, i: usize) {
        self.menu = false;
        self.mark_all();
        match i {
            0..=7 => self.open(APPS[i]),
            8 => self.text_mode(),
            9 => {
                self.open(App::Power);
                self.confirm = Some((0, self.now_ms()));
            }
            _ => {
                self.open(App::Power);
                self.confirm = Some((2, self.now_ms()));
            }
        }
    }

    fn power(&mut self, kind: u32) {
        let now = self.now_ms();
        match self.confirm {
            Some((k, t)) if k == kind && now.wrapping_sub(t) < 8000 => {
                self.log(
                    b"reset",
                    if kind == 0 {
                        b"kind=restart"
                    } else {
                        b"kind=poweroff"
                    },
                );
                // SAFETY: firmware runtime services; returns only if the reset was ignored.
                unsafe { self.c.reset(kind) };
                self.term.tone(Tone::Error);
                self.term.put(b"  reset did not happen\n");
                self.term.tone(Tone::Normal);
                self.confirm = None;
            }
            _ => self.confirm = Some((kind, now)),
        }
        let r = self.wins[App::Power.idx()].r;
        self.mark(Self::win_area(r));
    }

    /// Hand the screen to the firmware text console; come back when it says `desktop`.
    fn text_mode(&mut self) {
        self.log(b"text_mode", b"enter");
        // SAFETY: firmware ConOut/ConIn, boot services active.
        unsafe {
            self.ptr.suspend();
            console::text_console(self.c);
            self.c.text_cursor(false);
            self.ptr.resume();
        }
        self.btn = false;
        self.drag = None;
        self.log(b"text_mode", b"leave");
        self.mark_all();
    }

    fn run_command(&mut self) {
        let mut cmd = [0u8; 72];
        let n = self.term.ilen;
        cmd[..n].copy_from_slice(&self.term.input[..n]);
        self.term.ilen = 0;
        if n > 0 {
            self.last_cmd = cmd;
            self.last_len = n;
        }
        self.term.tone(Tone::Warn);
        self.term.put(b"asolaria> ");
        self.term.tone(Tone::Normal);
        console::text(&mut self.term, &cmd[..n]);
        self.term.put(b"\n");
        self.log(b"cmd", &cmd[..n]);
        match console::execute(&cmd[..n], &mut self.term, self.c) {
            Action::Clear => self.term.clear(),
            Action::Desktop => self.term.put(b"  already on the desktop\n"),
            Action::TextMode => self.text_mode(),
            Action::Reset(kind) => {
                // A typed reset is already explicit: no second confirmation.
                // SAFETY: firmware runtime services.
                unsafe { self.c.reset(kind) };
                self.term.tone(Tone::Error);
                self.term.put(b"  reset did not happen\n");
                self.term.tone(Tone::Normal);
            }
            Action::None => {}
        }
        let r = self.wins[App::Console.idx()].r;
        self.mark(Self::win_area(r));
    }

    fn scroll(&mut self, a: usize, delta: i32) {
        if a == App::Console.idx() {
            let max = self.term.used.saturating_sub(2) as i32;
            self.term.scroll = (self.term.scroll as i32 + delta).clamp(0, max) as usize;
        } else if a == App::Claude.idx() {
            let max = self.cterm.used.saturating_sub(2) as i32;
            self.cterm.scroll = (self.cterm.scroll as i32 + delta).clamp(0, max) as usize;
        } else if a == App::Devices.idx() {
            let n = self.c.facts.pci_devices as i32;
            self.wins[a].scroll = (self.wins[a].scroll - delta).clamp(0, (n - 1).max(0));
        } else {
            return;
        }
        let r = self.wins[a].r;
        self.mark(Self::win_area(r));
    }

    // ---------------------------------------------------------------- pointer input
    fn press(&mut self, x: i32, y: i32) {
        let h = self.hit(x, y);
        if self.menu && !matches!(h, Hit::Menu(_) | Hit::Start) {
            self.menu = false;
            self.mark_all();
            if matches!(h, Hit::None) {
                return;
            }
        }
        match h {
            Hit::Menu(i) => self.menu_activate(i),
            Hit::Start => {
                self.menu = !self.menu;
                self.menu_sel = 0;
                self.mark_all();
            }
            Hit::Task(a) => {
                if self.focused() == Some(a) {
                    self.minimize(a);
                } else {
                    self.raise(a);
                }
            }
            Hit::Close(a) => self.close(a),
            Hit::Min(a) => self.minimize(a),
            Hit::Title(a) => {
                self.raise(a);
                let r = self.wins[a].r;
                self.drag = Some((a, x - r.x, y - r.y));
            }
            Hit::Body(a) => {
                self.raise(a);
                if a == App::Power.idx() {
                    let (r1, r2, r3) = apps::power_buttons(Self::body(self.wins[a].r));
                    if r1.contains(x, y) {
                        self.power(0);
                    } else if r2.contains(x, y) {
                        self.power(2);
                    } else if r3.contains(x, y) {
                        self.text_mode();
                    }
                }
            }
            Hit::Icon(i) => {
                let now = self.now_ms();
                let double = self.last_click.1 == i && now.wrapping_sub(self.last_click.0) < 500;
                self.sel = i;
                self.desk_focus = true;
                self.last_click = (now, i);
                if double {
                    self.last_click = (0, usize::MAX);
                    self.open(APPS[i]);
                } else {
                    self.mark_all();
                }
            }
            Hit::Desktop => {
                self.desk_focus = true;
                self.mark_all();
            }
            Hit::Tray | Hit::None => {}
        }
    }

    fn pointer(&mut self, nx: i32, ny: i32, left: bool, dz: i32) {
        let nx = nx.clamp(0, self.scr.w - 1);
        let ny = ny.clamp(0, self.scr.h - 1);
        if (nx, ny) != (self.mx, self.my) {
            self.mx = nx;
            self.my = ny;
            if let Some((a, ox, oy)) = self.drag {
                let old = self.wins[a].r;
                let mut r = old;
                r.x = (nx - ox).clamp(-(r.w - 90), self.scr.w - 90);
                r.y = (ny - oy).clamp(0, self.scr.h - TB - TITLE);
                self.wins[a].r = r;
                self.mark(Self::win_area(old));
                self.mark(Self::win_area(r));
            } else {
                let h = self.hit(nx, ny);
                if h != self.hover {
                    let old = self.hit_rect(self.hover);
                    let new = self.hit_rect(h);
                    self.mark(old);
                    self.mark(new);
                    self.hover = h;
                }
            }
        }
        if left && !self.btn {
            self.btn = true;
            self.press(nx, ny);
        } else if !left && self.btn {
            self.btn = false;
            self.drag = None;
        }
        if dz != 0 {
            match self.hit(nx, ny) {
                Hit::Body(a) | Hit::Title(a) => self.scroll(a, dz * 3),
                _ => {}
            }
        }
    }

    // ---------------------------------------------------------------- keyboard input
    fn key(&mut self, sc: u16, ch: u16) {
        match sc {
            0x0B => return self.open(App::Help),
            0x0C => {
                self.menu = !self.menu;
                self.menu_sel = 0;
                return self.mark_all();
            }
            0x0D => return self.cycle(),
            0x0E => {
                if let Some(a) = self.focused() {
                    self.close(a);
                }
                return;
            }
            0x0F => {
                self.kptr = !self.kptr;
                if !self.kptr && self.kheld {
                    self.kheld = false;
                    self.pointer(self.mx, self.my, false, 0);
                }
                self.log(b"kbd_pointer", if self.kptr { b"on" } else { b"off" });
                return self.mark(self.tray_rect());
            }
            _ => {}
        }
        if sc == 0x17 {
            if self.menu {
                self.menu = false;
                self.mark_all();
            } else if self.kptr {
                self.key(0x0F, 0);
            } else if self.confirm.is_some() {
                self.confirm = None;
                self.mark_all();
            } else if let Some(a) = self.focused() {
                if a == App::Console.idx() {
                    self.term.ilen = 0;
                    self.mark(Self::win_area(self.wins[a].r));
                } else if a == App::Claude.idx() {
                    self.cterm.ilen = 0;
                    self.mark(Self::win_area(self.wins[a].r));
                } else {
                    self.close(a);
                }
            }
            return;
        }
        if self.menu {
            match (sc, ch) {
                (0x01, _) => self.menu_sel = (self.menu_sel + MENU_N - 1) % MENU_N,
                (0x02, _) | (_, 0x09) => self.menu_sel = (self.menu_sel + 1) % MENU_N,
                (_, 0x0D) => return self.menu_activate(self.menu_sel),
                _ => {}
            }
            return self.mark(self.menu_rect());
        }
        if self.kptr {
            let now = self.now_ms();
            if (0x01..=0x04).contains(&sc) {
                // Repeated presses of the same arrow accelerate: 8, 16, 32 px.
                let step = if self.arrow.0 == sc && now.wrapping_sub(self.arrow.1) < 220 {
                    (self.arrow.2 * 2).min(32)
                } else {
                    8
                };
                self.arrow = (sc, now, step);
                let (dx, dy) = match sc {
                    0x01 => (0, -step),
                    0x02 => (0, step),
                    0x03 => (step, 0),
                    _ => (-step, 0),
                };
                self.pointer(self.mx + dx, self.my + dy, self.kheld, 0);
            } else if ch == 0x0D {
                self.pointer(self.mx, self.my, true, 0);
                self.pointer(self.mx, self.my, self.kheld, 0);
            } else if ch == b' ' as u16 {
                self.kheld = !self.kheld;
                self.pointer(self.mx, self.my, self.kheld, 0);
            } else if sc == 0x09 || sc == 0x0A {
                self.pointer(
                    self.mx,
                    self.my,
                    self.kheld,
                    if sc == 0x09 { 1 } else { -1 },
                );
            }
            return;
        }
        if ch == 0x09 {
            return self.cycle();
        }
        match self.focused() {
            Some(a) => self.app_key(a, sc, ch),
            None => self.desktop_key(sc, ch),
        }
    }

    fn desktop_key(&mut self, sc: u16, ch: u16) {
        let pc = self.per_col() as usize;
        let s = self.sel;
        self.sel = match sc {
            0x01 => s.saturating_sub(1),
            0x02 => (s + 1).min(NAPP - 1),
            0x03 => (s + pc).min(NAPP - 1),
            0x04 => s.saturating_sub(pc),
            _ => s,
        };
        if ch == 0x0D {
            return self.open(APPS[self.sel]);
        }
        if self.sel != s {
            self.mark(self.icon_cell(s));
            self.mark(self.icon_cell(self.sel));
        }
    }

    fn app_key(&mut self, a: usize, sc: u16, ch: u16) {
        let app = APPS[a];
        match app {
            App::Console => {
                match (sc, ch) {
                    (0x09, _) => return self.scroll(a, 8),
                    (0x0A, _) => return self.scroll(a, -8),
                    (0x01, _) => {
                        self.term.input = self.last_cmd;
                        self.term.ilen = self.last_len;
                    }
                    (_, 0x0D) => return self.run_command(),
                    (_, 0x08) => self.term.ilen = self.term.ilen.saturating_sub(1),
                    (_, c)
                        if (0x20..0x7F).contains(&c) && self.term.ilen < self.term.input.len() =>
                    {
                        self.term.input[self.term.ilen] = c as u8;
                        self.term.ilen += 1;
                    }
                    _ => return,
                }
                self.caret = true;
                self.term.scroll = 0;
                let r = self.wins[a].r;
                self.mark(Self::win_area(r));
            }
            App::Claude => {
                match (sc, ch) {
                    (0x09, _) => return self.scroll(a, 8),
                    (0x0A, _) => return self.scroll(a, -8),
                    (_, 0x0D) => return self.claude_submit(),
                    (_, 0x08) => self.cterm.ilen = self.cterm.ilen.saturating_sub(1),
                    (_, c)
                        if (0x20..0x7F).contains(&c)
                            && self.cterm.ilen < self.cterm.input.len() =>
                    {
                        self.cterm.input[self.cterm.ilen] = c as u8;
                        self.cterm.ilen += 1;
                    }
                    _ => return,
                }
                self.caret = true;
                self.cterm.scroll = 0;
                let r = self.wins[a].r;
                self.mark(Self::win_area(r));
            }
            App::Devices => match sc {
                0x01 => self.scroll(a, 1),
                0x02 => self.scroll(a, -1),
                0x09 => self.scroll(a, 10),
                0x0A => self.scroll(a, -10),
                0x05 => self.scroll(a, 999),
                _ => {}
            },
            App::Power => match ch | 0x20 {
                0x72 => self.power(0),    // 'r'
                0x70 => self.power(2),    // 'p'
                0x74 => self.text_mode(), // 't'
                0x2D if ch == 0x0D => {
                    // Enter: confirm whatever is pending.
                    if let Some((k, _)) = self.confirm {
                        self.power(k);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    // ---------------------------------------------------------------- drawing
    fn compose(&mut self) {
        let clip = self.dirty.intersect(&self.cv.bounds());
        if clip.is_empty() {
            return;
        }
        let old = self.cv.set_clip(clip);
        match self.wall.as_ref() {
            Some(w) => self.cv.copy_from(w, clip),
            None => {
                let b = self.cv.bounds();
                self.cv.vgrad(b, apps::BG_TOP, apps::BG_BOT);
            }
        }
        // Desktop icons.
        for (i, app) in APPS.iter().enumerate() {
            let cell = self.icon_cell(i);
            if cell.intersect(&clip).is_empty() {
                continue;
            }
            let selected = self.sel == i && self.desk_focus;
            if selected {
                self.cv.round(cell, 10, apps::CYAN, 46);
            } else if self.hover == Hit::Icon(i) {
                self.cv.round(cell, 10, apps::WHITE, 20);
            }
            apps::draw_icon(&mut self.cv, *app, cell.x + 26, cell.y + 8);
            let col = if selected { apps::WHITE } else { apps::TEXT };
            self.cv
                .text_center((cell.x, cell.w), cell.y + 64, app.name(), col, 1, true);
        }
        // Windows, bottom to top.
        let top = self.focused();
        for k in 0..self.nz {
            let a = self.z[k];
            let w = self.wins[a];
            if w.min || Self::win_area(w.r).intersect(&clip).is_empty() {
                continue;
            }
            self.draw_window(a, top == Some(a), clip);
        }
        self.draw_taskbar(clip);
        if self.menu {
            self.draw_menu();
        }
        self.cv.restore_clip(old);
    }

    fn draw_window(&mut self, a: usize, focused: bool, clip: Rect) {
        let note = self.ptr_note();
        let app = APPS[a];
        let w = self.wins[a];
        let r = w.r;
        let cv = &mut self.cv;
        cv.round(
            Rect::new(r.x + 3, r.y + 8, r.w + 4, r.h + 4),
            14,
            apps::BLACK,
            60,
        );
        cv.round(
            Rect::new(r.x + 1, r.y + 3, r.w + 2, r.h + 2),
            12,
            apps::BLACK,
            70,
        );
        cv.round(
            r,
            10,
            if focused {
                mix(apps::LINE, app.color(), 90)
            } else {
                apps::LINE
            },
            256,
        );
        cv.round(r.inset(1), 9, apps::PANEL, 256);
        let (t0, t1) = if focused {
            (rgb(0x22, 0x31, 0x55), rgb(0x14, 0x1e, 0x36))
        } else {
            (rgb(0x18, 0x21, 0x36), rgb(0x10, 0x17, 0x27))
        };
        cv.round_vgrad(Rect::new(r.x + 1, r.y + 1, r.w - 2, TITLE + 8), 9, t0, t1);
        cv.fill(Rect::new(r.x + 1, r.y + TITLE, r.w - 2, 9), apps::PANEL);
        cv.fill(
            Rect::new(r.x + 10, r.y + TITLE - 1, r.w - 20, 1),
            if focused { app.color() } else { apps::LINE },
        );
        cv.disc(r.x + 18, r.y + 16, 5, app.color());
        let tc = if focused { apps::WHITE } else { apps::DIM };
        cv.text(r.x + 32, r.y + 8, app.name(), tc, 1);
        let cr = Self::close_rect(r);
        let hov_c = self.hover == Hit::Close(a);
        cv.round(
            cr,
            6,
            if hov_c {
                apps::RED
            } else {
                mix(apps::PANEL2, apps::RED, 70)
            },
            256,
        );
        cv.line_w(cr.x + 8, cr.y + 5, cr.x + 15, cr.y + 12, 2, apps::WHITE);
        cv.line_w(cr.x + 15, cr.y + 5, cr.x + 8, cr.y + 12, 2, apps::WHITE);
        let mr = Self::min_rect(r);
        let hov_m = self.hover == Hit::Min(a);
        cv.round(
            mr,
            6,
            if hov_m {
                mix(apps::PANEL2, apps::CYAN, 110)
            } else {
                apps::PANEL2
            },
            256,
        );
        cv.fill(Rect::new(mr.x + 7, mr.y + 11, 10, 2), apps::TEXT);
        // Body, clipped to itself.
        let body = Self::body(r);
        let old = cv.set_clip(body.intersect(&clip));
        let now = self.now;
        let v = View {
            facts: self.c.facts,
            wd: self.c.wd,
            gop_w: self.scr.w,
            gop_h: self.scr.h,
            gop_fmt: self.scr.fmt,
            gop_pps: self.scr.pps,
            ptr_simple: self.ptr.counts().0,
            ptr_abs: self.ptr.counts().1,
            ptr_handles: (self.ptr.simple_handles, self.ptr.abs_handles),
            ptr_splitter: self.ptr.via_splitter,
            usb_connected: self.ptr.connected_usb,
            ptr_note: note,
            bridge_note: self.bridge.state.text(),
            bridge_ok: self.bridge.state == BridgeState::Connected,
            claude_busy: self.claude_busy,
            i2c: self.ptr.i2c_report,
            kbd_pointer: self.kptr,
            mem: self.mem,
            tsc_khz: self.tpms,
            uptime_s: (rtc::rdtsc().wrapping_sub(self.t0)) / self.tpms / 1000,
            now: &now,
            fw_vendor: &self.fw[..self.fw_len],
            fw_rev: self.fw_rev,
            caret: self.caret,
            focused,
            power_confirm: self.confirm.map(|c| c.0),
        };
        apps::draw_body(cv, app, body, &v, (&self.term, &self.cterm), w.scroll);
        cv.restore_clip(old);
    }

    fn draw_taskbar(&mut self, clip: Rect) {
        let (w, h) = (self.scr.w, self.scr.h);
        let tb = Rect::new(0, h - TB, w, TB);
        if tb.intersect(&clip).is_empty() {
            return;
        }
        self.cv.fill_blend(tb, rgb(0x04, 0x07, 0x0e), 215);
        self.cv.fill(Rect::new(0, h - TB, w, 1), apps::LINE);
        // Start button.
        let s = self.start_rect();
        let lit = self.menu || self.hover == Hit::Start;
        let (a, b) = if lit {
            (rgb(0x1f, 0x6f, 0x7a), rgb(0x10, 0x34, 0x40))
        } else {
            (rgb(0x1a, 0x28, 0x46), rgb(0x0e, 0x16, 0x2a))
        };
        self.cv.round_vgrad(s, 8, a, b);
        apps::iso_cube(
            &mut self.cv,
            s.x + 20,
            s.y + 16,
            9,
            256,
            rgb(0x05, 0x10, 0x18),
        );
        self.cv.text(s.x + 38, s.y + 8, b"Asolaria", apps::WHITE, 1);
        // Task buttons.
        let top = self.focused();
        for app in APPS {
            let i = app.idx();
            let Some(r) = self.task_rect(i) else { continue };
            let foc = top == Some(i);
            let min = self.wins[i].min;
            let bg = if foc {
                mix(apps::PANEL2, app.color(), 60)
            } else if self.hover == Hit::Task(i) {
                mix(apps::PANEL2, apps::WHITE, 24)
            } else {
                apps::PANEL2
            };
            self.cv.round(r, 7, bg, 256);
            let ind = if foc { r.w - 20 } else { 18 };
            self.cv.fill(
                Rect::new(r.x + (r.w - ind) / 2, r.y + r.h - 3, ind, 2),
                app.color(),
            );
            self.cv.disc(r.x + 14, r.y + r.h / 2, 4, app.color());
            let tc = if min { apps::DIM } else { apps::TEXT };
            self.cv.text(r.x + 26, r.y + 8, app.name(), tc, 1);
        }
        // Tray: status pills + clock.
        let mut x = w - 104;
        let t = self.now;
        let mut b = Buf::new();
        b.d2(t.hour).s(b":").d2(t.min).s(b":").d2(t.sec);
        self.cv.text(x + 16, h - TB + 6, b.get(), apps::WHITE, 1);
        let mut d = Buf::new();
        d.dec(t.year as u64).s(b"-").d2(t.month).s(b"-").d2(t.day);
        self.cv.text(x + 12, h - TB + 23, d.get(), apps::DIM, 1);
        let (ns, na) = self.ptr.counts();
        let pill_ptr: (&[u8], u32) = if self.kptr {
            (b"kbd pointer", apps::AMBER)
        } else if ns + na > 0 {
            (b"mouse", apps::GREEN)
        } else {
            (b"no mouse: F5", apps::AMBER)
        };
        let wd_ok = matches!(
            self.c.wd,
            console::Watchdog::Disabled | console::Watchdog::Unsupported
        );
        let pills: [(&[u8], u32); 3] = [
            (b"E=0", apps::GREEN),
            (
                if wd_ok { b"wd off" } else { b"wd ON" },
                if wd_ok { apps::GREEN } else { apps::RED },
            ),
            pill_ptr,
        ];
        for (label, c) in pills.iter().rev() {
            let pw = text_w(label, 1) + 18;
            x -= pw + 8;
            let r = Rect::new(x, h - TB + 11, pw, 22);
            self.cv.round(r, 11, mix(apps::PANEL2, *c, 40), 256);
            self.cv.disc(r.x + 9, r.y + 11, 3, *c);
            self.cv.text(r.x + 15, r.y + 3, label, apps::TEXT, 1);
        }
    }

    fn draw_menu(&mut self) {
        let m = self.menu_rect();
        let cv = &mut self.cv;
        cv.round(
            Rect::new(m.x + 3, m.y + 8, m.w + 4, m.h + 4),
            14,
            apps::BLACK,
            70,
        );
        cv.round(m, 12, apps::LINE, 256);
        cv.round(m.inset(1), 11, rgb(0x0b, 0x11, 0x20), 256);
        cv.round_vgrad(
            Rect::new(m.x + 1, m.y + 1, m.w - 2, 56),
            11,
            rgb(0x14, 0x3e, 0x4e),
            rgb(0x0b, 0x11, 0x20),
        );
        apps::iso_cube(cv, m.x + 30, m.y + 30, 16, 256, rgb(0x05, 0x10, 0x18));
        cv.text(m.x + 58, m.y + 12, b"ASOLARIA ASI OS", apps::CYAN, 1);
        cv.text(m.x + 58, m.y + 31, b"federation-1024  .  E=0", apps::DIM, 1);
        let labels: [&[u8]; MENU_N] = [
            b"Console",
            b"System",
            b"Devices",
            b"Clock",
            b"Power",
            b"About",
            b"Help  (F1)",
            b"Claude  (Opus 5.5)",
            b"Text console",
            b"Restart...",
            b"Power off...",
        ];
        for (i, label) in labels.iter().enumerate() {
            let r = self.menu_item(i);
            if i == self.menu_sel || self.hover == Hit::Menu(i) {
                self.cv
                    .round(r, 7, mix(rgb(0x0b, 0x11, 0x20), apps::CYAN, 60), 256);
            }
            let c = match i.cmp(&NAPP) {
                core::cmp::Ordering::Less => APPS[i].color(),
                core::cmp::Ordering::Equal => apps::TEXT,
                core::cmp::Ordering::Greater => apps::RED,
            };
            self.cv.disc(r.x + 16, r.y + 14, 5, c);
            self.cv.text(r.x + 32, r.y + 6, label, apps::TEXT, 1);
        }
        let sep = self.menu_item(NAPP).y - 6;
        self.cv
            .fill(Rect::new(m.x + 12, sep, m.w - 24, 1), apps::LINE);
    }

    /// Present the dirty region and keep the cursor on top.
    fn frame(&mut self) {
        let moved = (self.mx, self.my) != self.drawn;
        if self.dirty.is_empty() && !moved {
            return;
        }
        if !self.dirty.is_empty() {
            self.compose();
        }
        let old_cur = Rect::new(self.drawn.0, self.drawn.1, CURSOR_W, CURSOR_H);
        let region = self.dirty.union(&old_cur);
        // SAFETY: framebuffer from GOP, bounds-clamped inside `present`.
        unsafe {
            self.scr.present(&self.cv, region);
            self.scr.cursor(self.mx, self.my, self.kptr);
        }
        self.drawn = (self.mx, self.my);
        self.dirty = Rect::EMPTY;
    }

    fn event_loop(&mut self) {
        loop {
            self.claude_pump();
            // Keyboard: drain what is waiting (bounded per frame).
            for _ in 0..16 {
                // SAFETY: firmware ConIn.
                let Some(k) = (unsafe { self.c.key() }) else {
                    break;
                };
                self.key(k.scan_code, k.unicode_char);
            }
            // Pointer devices.
            // SAFETY: firmware pointer protocols, boot services active.
            if let Some(ev) = unsafe { self.ptr.poll(self.scr.w, self.scr.h) } {
                let (nx, ny) = match ev.abs {
                    Some(p) => p,
                    None => (self.mx + ev.dx, self.my + ev.dy),
                };
                if !self.motion_logged && (ev.dx != 0 || ev.dy != 0 || ev.abs.is_some()) {
                    self.motion_logged = true;
                    let src: &[u8] = if ev.abs.is_some() {
                        b"src=absolute"
                    } else if self.ptr.i2c_active() {
                        b"src=i2c_hid"
                    } else if self.ptr.ps2_active() {
                        b"src=ps2"
                    } else {
                        b"src=simple"
                    };
                    self.log(b"pointer_first_motion", src);
                }
                let held = ev.left || (self.kptr && self.kheld);
                self.pointer(nx, ny, held, ev.dz);
            }
            let now = self.now_ms();
            if now.wrapping_sub(self.last_rtc) >= 250 {
                self.last_rtc = now;
                // SAFETY: CMOS port I/O, read-only.
                let t = unsafe { rtc::now_bounded() }.unwrap_or(self.now);
                if t.sec != self.now.sec {
                    self.now = t;
                    self.mark(self.tray_rect());
                    for app in [App::Clock, App::System] {
                        let w = self.wins[app.idx()];
                        if w.open && !w.min {
                            self.mark(Self::win_area(w.r));
                        }
                    }
                }
            }
            if now.wrapping_sub(self.last_blink) >= 530 {
                self.last_blink = now;
                self.caret = !self.caret;
                for app in [App::Console, App::Claude] {
                    let w = self.wins[app.idx()];
                    if w.open && !w.min && self.focused() == Some(app.idx()) {
                        self.mark(Self::body(w.r));
                    }
                }
            }
            if let Some((_, t)) = self.confirm {
                if now.wrapping_sub(t) >= 8000 {
                    self.confirm = None;
                    self.mark(Self::win_area(self.wins[App::Power.idx()].r));
                }
            }
            self.frame();
            // SAFETY: firmware Stall.
            unsafe { Bs((*self.c.st).boot_services as *mut BootServices).stall(4_000) };
        }
    }
}
