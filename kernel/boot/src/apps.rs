//! Desktop applications: icons, window contents, the Console terminal model, and the
//! vendor / PCI-class name tables. Drawing only — input routing lives in `gui`.
//!
//! Every number shown is read from the machine (PCI config space, GOP mode, UEFI memory map,
//! CMOS RTC, TSC) or from the kernel's own constants; nothing is invented for decoration.

use crate::console::{self, BootFacts, Sink, Tone, Watchdog};
use crate::gfx::{line_h, mix, rgb, text_w, Canvas, Rect};
use crate::rtc::BootTime;

// ------------------------------------------------------------------ palette
pub(crate) const BG_TOP: u32 = rgb(0x0b, 0x14, 0x2a);
pub(crate) const BG_BOT: u32 = rgb(0x03, 0x05, 0x0c);
pub(crate) const PANEL: u32 = rgb(0x0c, 0x13, 0x22);
pub(crate) const PANEL2: u32 = rgb(0x13, 0x1d, 0x33);
pub(crate) const LINE: u32 = rgb(0x26, 0x34, 0x4f);
pub(crate) const CYAN: u32 = rgb(0x43, 0xe8, 0xd8);
pub(crate) const AMBER: u32 = rgb(0xff, 0xb4, 0x54);
pub(crate) const VIOLET: u32 = rgb(0xa7, 0x8b, 0xfa);
pub(crate) const GREEN: u32 = rgb(0x5c, 0xe0, 0xa0);
pub(crate) const RED: u32 = rgb(0xff, 0x6b, 0x6b);
pub(crate) const BLUE: u32 = rgb(0x5a, 0x9b, 0xff);
pub(crate) const TEXT: u32 = rgb(0xdc, 0xe6, 0xf8);
pub(crate) const DIM: u32 = rgb(0x7a, 0x8a, 0xa8);
pub(crate) const BLACK: u32 = rgb(0, 0, 0);
pub(crate) const WHITE: u32 = rgb(0xff, 0xff, 0xff);

// ------------------------------------------------------------------ applications
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum App {
    Console,
    System,
    Devices,
    Clock,
    Power,
    About,
    Help,
    Claude,
}

pub(crate) const APPS: [App; 8] = [
    App::Console,
    App::System,
    App::Devices,
    App::Clock,
    App::Power,
    App::About,
    App::Help,
    App::Claude,
];

impl App {
    pub(crate) fn idx(self) -> usize {
        match self {
            App::Console => 0,
            App::System => 1,
            App::Devices => 2,
            App::Clock => 3,
            App::Power => 4,
            App::About => 5,
            App::Help => 6,
            App::Claude => 7,
        }
    }
    pub(crate) fn name(self) -> &'static [u8] {
        match self {
            App::Console => b"Console",
            App::System => b"System",
            App::Devices => b"Devices",
            App::Clock => b"Clock",
            App::Power => b"Power",
            App::About => b"About",
            App::Help => b"Help",
            App::Claude => b"Claude",
        }
    }
    pub(crate) fn color(self) -> u32 {
        match self {
            App::Console => CYAN,
            App::System => VIOLET,
            App::Devices => GREEN,
            App::Clock => AMBER,
            App::Power => RED,
            App::About => CYAN,
            App::Help => BLUE,
            App::Claude => AMBER,
        }
    }
    /// Default window size (clamped to the screen by the window manager).
    pub(crate) fn size(self) -> (i32, i32) {
        match self {
            App::Console => (720, 440),
            App::System => (560, 400),
            App::Devices => (760, 480),
            App::Clock => (380, 330),
            App::Power => (540, 250),
            App::About => (700, 330),
            App::Help => (740, 440),
            App::Claude => (780, 460),
        }
    }
}

// ------------------------------------------------------------------ small text buffer
/// Fixed-capacity ASCII line builder (no heap).
pub(crate) struct Buf {
    pub b: [u8; 200],
    pub n: usize,
}

impl Buf {
    pub(crate) fn new() -> Self {
        Buf { b: [0; 200], n: 0 }
    }
    pub(crate) fn s(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = if (0x20..0x7F).contains(&c) { c } else { b'.' };
                self.n += 1;
            }
        }
        self
    }
    /// Append bytes verbatim (line endings for serial rows).
    pub(crate) fn raw(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        self
    }
    pub(crate) fn dec(&mut self, v: u64) -> &mut Self {
        let mut t = [0u8; 20];
        let k = console::fmt_dec(&mut t, v);
        self.s(&t[20 - k..])
    }
    /// Zero-padded two-digit decimal.
    pub(crate) fn d2(&mut self, v: u8) -> &mut Self {
        self.s(&[b'0' + (v / 10) % 10, b'0' + v % 10])
    }
    pub(crate) fn hex(&mut self, bytes: &[u8]) -> &mut Self {
        const H: &[u8; 16] = b"0123456789abcdef";
        for &x in bytes {
            self.s(&[H[(x >> 4) as usize], H[(x & 15) as usize]]);
        }
        self
    }
    pub(crate) fn hex16(&mut self, v: u16) -> &mut Self {
        self.hex(&v.to_be_bytes())
    }
    /// Pad with spaces up to column `col`.
    pub(crate) fn pad(&mut self, col: usize) -> &mut Self {
        while self.n < col && self.n < self.b.len() {
            self.b[self.n] = b' ';
            self.n += 1;
        }
        self
    }
    pub(crate) fn get(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

// ------------------------------------------------------------------ terminal model
/// The Console window's text grid (scrollback ring of `rows` x `cols`), a [`Sink`].
pub(crate) struct Term {
    cells: &'static mut [u8],
    tones: &'static mut [u8],
    pub cols: usize,
    pub rows: usize,
    /// Lines written so far (the cursor line is `used - 1`), capped at `rows`.
    pub used: usize,
    col: usize,
    tone: u8,
    pub input: [u8; 72],
    pub ilen: usize,
    pub scroll: usize,
}

impl Term {
    pub(crate) fn new(cells: &'static mut [u8], tones: &'static mut [u8], cols: usize) -> Term {
        let rows = cells.len() / cols;
        Term {
            cells,
            tones,
            cols,
            rows,
            used: 1,
            col: 0,
            tone: 0,
            input: [0; 72],
            ilen: 0,
            scroll: 0,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.cells.fill(b' ');
        self.tones.fill(0);
        self.used = 1;
        self.col = 0;
        self.scroll = 0;
    }

    fn newline(&mut self) {
        self.col = 0;
        if self.used < self.rows {
            self.used += 1;
        } else {
            // Scroll the ring up one line.
            let c = self.cols;
            self.cells.copy_within(c.., 0);
            self.tones.copy_within(c.., 0);
            let n = self.cells.len();
            self.cells[n - c..].fill(b' ');
            self.tones[n - c..].fill(0);
        }
    }

    pub(crate) fn line(&self, i: usize) -> (&[u8], &[u8]) {
        let a = i * self.cols;
        (&self.cells[a..a + self.cols], &self.tones[a..a + self.cols])
    }
}

impl Sink for Term {
    fn put(&mut self, s: &[u8]) {
        for &ch in s {
            if ch == b'\n' {
                self.newline();
                continue;
            }
            if self.col >= self.cols {
                self.newline();
            }
            let i = (self.used - 1) * self.cols + self.col;
            self.cells[i] = if (0x20..0x7F).contains(&ch) { ch } else { b'.' };
            self.tones[i] = self.tone;
            self.col += 1;
        }
        self.scroll = 0;
    }
    fn tone(&mut self, t: Tone) {
        self.tone = match t {
            Tone::Normal => 0,
            Tone::Accent => 1,
            Tone::Warn => 2,
            Tone::Error => 3,
        };
    }
}

fn tone_color(t: u8) -> u32 {
    match t {
        1 => CYAN,
        2 => AMBER,
        3 => RED,
        _ => TEXT,
    }
}

// ------------------------------------------------------------------ what the apps can see
/// Read-only facts the windows draw from, assembled by the desktop each frame.
pub(crate) struct View<'a> {
    pub facts: &'a BootFacts,
    pub wd: Watchdog,
    pub gop_w: i32,
    pub gop_h: i32,
    pub gop_fmt: u32,
    pub gop_pps: i32,
    pub ptr_simple: usize,
    pub ptr_abs: usize,
    pub ptr_handles: (usize, usize),
    pub ptr_splitter: bool,
    pub usb_connected: usize,
    /// Which driver feeds the pointer (firmware devices, the kernel's PS/2 driver, or none).
    pub ptr_note: &'static [u8],
    /// Claude-terminal bridge state line and whether a question is in flight.
    pub bridge_note: &'static [u8],
    pub bridge_ok: bool,
    pub claude_busy: bool,
    /// What the kernel's I2C-HID touchpad probe saw (None when firmware devices were found).
    pub i2c: Option<crate::i2chid::Report>,
    pub kbd_pointer: bool,
    pub mem: Option<(u64, u64)>,
    pub tsc_khz: u64,
    pub uptime_s: u64,
    pub now: &'a BootTime,
    pub fw_vendor: &'a [u8],
    pub fw_rev: u32,
    pub caret: bool,
    pub focused: bool,
    pub power_confirm: Option<u32>,
}

// ------------------------------------------------------------------ icons
/// Isometric 3x3x3 cube — the Asolaria mark (27 = 3^3). `s` = edge length in pixels.
pub(crate) fn iso_cube(cv: &mut Canvas, cx: i32, cy: i32, s: i32, t: u32, lines: u32) {
    let a = s * 866 / 1000; // cos 30 deg, integer
    let h = s / 2;
    let top = [(cx, cy - s), (cx + a, cy - h), (cx, cy), (cx - a, cy - h)];
    let left = [(cx - a, cy - h), (cx, cy), (cx, cy + s), (cx - a, cy + h)];
    let right = [(cx, cy), (cx + a, cy - h), (cx + a, cy + h), (cx, cy + s)];
    cv.fill_convex(&top, rgb(0x6c, 0xf5, 0xe6), t);
    cv.fill_convex(&left, rgb(0x2a, 0x8f, 0xb8), t);
    cv.fill_convex(&right, rgb(0x6d, 0x52, 0xd6), t);
    // The 3x3 grid on each visible face.
    for k in 1..3 {
        // top face: lines parallel to its two edge directions
        let (p0, p1) = (lerp(top[0], top[3], k), lerp(top[1], top[2], k));
        cv.line(p0.0, p0.1, p1.0, p1.1, lines);
        let (p0, p1) = (lerp(top[0], top[1], k), lerp(top[3], top[2], k));
        cv.line(p0.0, p0.1, p1.0, p1.1, lines);
        // left face
        let (p0, p1) = (lerp(left[0], left[1], k), lerp(left[3], left[2], k));
        cv.line(p0.0, p0.1, p1.0, p1.1, lines);
        let (p0, p1) = (lerp(left[0], left[3], k), lerp(left[1], left[2], k));
        cv.line(p0.0, p0.1, p1.0, p1.1, lines);
        // right face
        let (p0, p1) = (lerp(right[0], right[1], k), lerp(right[3], right[2], k));
        cv.line(p0.0, p0.1, p1.0, p1.1, lines);
        let (p0, p1) = (lerp(right[0], right[3], k), lerp(right[1], right[2], k));
        cv.line(p0.0, p0.1, p1.0, p1.1, lines);
    }
    // Silhouette.
    for (p, q) in [
        (top[0], top[1]),
        (top[1], right[2]),
        (right[2], right[3]),
        (right[3], left[3]),
        (left[3], top[3]),
        (top[3], top[0]),
        (top[2], top[1]),
        (top[2], top[3]),
        (top[2], right[3]),
    ] {
        cv.line(p.0, p.1, q.0, q.1, lines);
    }
}

fn lerp(a: (i32, i32), b: (i32, i32), k: i32) -> (i32, i32) {
    (a.0 + (b.0 - a.0) * k / 3, a.1 + (b.1 - a.1) * k / 3)
}

/// Integer sine x1000 for minute positions 0..60 (6 degrees each).
pub(crate) fn sin60(i: i32) -> i32 {
    const Q: [i32; 16] = [
        0, 105, 208, 309, 407, 500, 588, 669, 743, 809, 866, 914, 951, 978, 995, 1000,
    ];
    let i = i.rem_euclid(60);
    match i {
        0..=15 => Q[i as usize],
        16..=30 => Q[(30 - i) as usize],
        31..=45 => -Q[(i - 30) as usize],
        _ => -Q[(60 - i) as usize],
    }
}

pub(crate) fn cos60(i: i32) -> i32 {
    sin60(i + 15)
}

/// A 48x48 desktop icon for `app` with its top-left at (x,y).
pub(crate) fn draw_icon(cv: &mut Canvas, app: App, x: i32, y: i32) {
    let r = Rect::new(x, y, 48, 48);
    // Soft drop shadow under every tile.
    cv.round(Rect::new(x + 2, y + 3, 48, 48), 11, BLACK, 90);
    match app {
        App::Console => {
            cv.round_vgrad(r, 10, rgb(0x1d, 0x2b, 0x48), rgb(0x0a, 0x10, 0x1e));
            cv.fill(Rect::new(x + 6, y + 9, 36, 1), rgb(0x2c, 0x3d, 0x60));
            cv.disc(x + 9, y + 5, 2, RED);
            cv.disc(x + 15, y + 5, 2, AMBER);
            cv.disc(x + 21, y + 5, 2, GREEN);
            cv.line_w(x + 10, y + 18, x + 18, y + 25, 3, CYAN);
            cv.line_w(x + 18, y + 25, x + 10, y + 32, 3, CYAN);
            cv.fill(Rect::new(x + 22, y + 31, 14, 3), CYAN);
        }
        App::System => {
            cv.round_vgrad(r, 10, rgb(0x3a, 0x2c, 0x7a), rgb(0x1a, 0x12, 0x40));
            let chip = Rect::new(x + 14, y + 14, 20, 20);
            for k in 0..4 {
                let o = 16 + k * 5;
                cv.fill(Rect::new(x + o, y + 9, 2, 5), VIOLET);
                cv.fill(Rect::new(x + o, y + 34, 2, 5), VIOLET);
                cv.fill(Rect::new(x + 9, y + o, 5, 2), VIOLET);
                cv.fill(Rect::new(x + 34, y + o, 5, 2), VIOLET);
            }
            cv.round(chip, 3, rgb(0xd9, 0xcf, 0xff), 256);
            cv.round(chip.inset(4), 2, rgb(0x6d, 0x52, 0xd6), 256);
        }
        App::Devices => {
            cv.round_vgrad(r, 10, rgb(0x14, 0x4d, 0x3a), rgb(0x08, 0x24, 0x1b));
            // an expansion card with gold edge contacts and two chips
            cv.round(
                Rect::new(x + 8, y + 12, 32, 20),
                3,
                rgb(0x2f, 0xa8, 0x6e),
                256,
            );
            cv.fill(Rect::new(x + 12, y + 16, 9, 8), rgb(0x10, 0x2a, 0x20));
            cv.fill(Rect::new(x + 24, y + 16, 12, 8), rgb(0x10, 0x2a, 0x20));
            for k in 0..6 {
                cv.fill(Rect::new(x + 11 + k * 5, y + 32, 3, 6), AMBER);
            }
        }
        App::Clock => {
            cv.round_vgrad(r, 10, rgb(0x5a, 0x3a, 0x10), rgb(0x2a, 0x1a, 0x06));
            cv.disc(x + 24, y + 24, 17, rgb(0xff, 0xf3, 0xde));
            cv.ring(x + 24, y + 24, 17, 2, AMBER);
            for k in 0..12 {
                let m = k * 5;
                cv.put(
                    x + 24 + sin60(m) * 14 / 1000,
                    y + 24 - cos60(m) * 14 / 1000,
                    rgb(0x60, 0x40, 0x10),
                );
            }
            cv.line_w(x + 24, y + 24, x + 24, y + 13, 2, rgb(0x30, 0x20, 0x08));
            cv.line_w(x + 24, y + 24, x + 32, y + 28, 2, rgb(0x30, 0x20, 0x08));
            cv.disc(x + 24, y + 24, 2, RED);
        }
        App::Power => {
            cv.round_vgrad(r, 10, rgb(0x6a, 0x1c, 0x24), rgb(0x30, 0x0a, 0x10));
            cv.ring(x + 24, y + 26, 13, 3, rgb(0xff, 0xd0, 0xd0));
            cv.fill(Rect::new(x + 19, y + 8, 10, 14), rgb(0x52, 0x14, 0x1c));
            cv.fill(Rect::new(x + 22, y + 9, 4, 17), rgb(0xff, 0xd0, 0xd0));
        }
        App::About => {
            cv.round_vgrad(r, 10, rgb(0x10, 0x3a, 0x4a), rgb(0x06, 0x16, 0x22));
            iso_cube(cv, x + 24, y + 25, 15, 256, rgb(0x05, 0x10, 0x18));
        }
        App::Help => {
            cv.round_vgrad(r, 10, rgb(0x1a, 0x40, 0x8a), rgb(0x0a, 0x1a, 0x40));
            cv.disc(x + 24, y + 24, 16, rgb(0xe6, 0xf0, 0xff));
            cv.text(x + 16, y + 9, b"?", rgb(0x1a, 0x40, 0x8a), 2);
        }
        App::Claude => {
            // A terminal tile in Claude's amber with a spark: the Opus 5.5 terminal.
            cv.round_vgrad(r, 10, rgb(0x5a, 0x34, 0x14), rgb(0x24, 0x12, 0x06));
            cv.line_w(x + 10, y + 16, x + 18, y + 23, 3, AMBER);
            cv.line_w(x + 18, y + 23, x + 10, y + 30, 3, AMBER);
            cv.fill(Rect::new(x + 22, y + 29, 12, 3), AMBER);
            cv.line_w(x + 36, y + 8, x + 36, y + 20, 2, WHITE);
            cv.line_w(x + 30, y + 14, x + 42, y + 14, 2, WHITE);
        }
    }
    // Glossy top highlight.
    cv.round(Rect::new(x + 3, y + 2, 42, 10), 5, WHITE, 26);
}

// ------------------------------------------------------------------ window bodies
fn kv(cv: &mut Canvas, x: i32, y: i32, k: &[u8], v: &[u8], vc: u32) {
    cv.text(x, y, k, DIM, 1);
    cv.text(x + 150, y, v, vc, 1);
}

/// Draw the body of `app` into `body` (already filled with the panel colour by the caller).
pub(crate) fn draw_body(
    cv: &mut Canvas,
    app: App,
    body: Rect,
    v: &View,
    terms: (&Term, &Term),
    scroll: i32,
) {
    let term = terms.0;
    let x = body.x + 16;
    let mut y = body.y + 14;
    let lh = line_h(1) + 4;
    match app {
        App::Console => draw_console(cv, body, term, v, b"asolaria> ", None),
        App::Claude => draw_console(
            cv,
            body,
            terms.1,
            v,
            b"opus> ",
            Some((
                if v.claude_busy {
                    b"thinking..." as &[u8]
                } else {
                    v.bridge_note
                },
                if v.bridge_ok { GREEN } else { AMBER },
            )),
        ),
        App::System => {
            let mut b = Buf::new();
            kv(
                cv,
                x,
                y,
                b"Kernel",
                asolaria_kernel_core::KERNEL_VERSION.as_bytes(),
                CYAN,
            );
            y += lh;
            kv(
                cv,
                x,
                y,
                b"Anchor",
                asolaria_kernel_core::FEDERATION_ANCHOR_PID.as_bytes(),
                TEXT,
            );
            y += lh;
            b.hex(&v.facts.device_digest[..8]);
            kv(cv, x, y, b"device_pid", b.get(), TEXT);
            y += lh;
            b = Buf::new();
            b.hex(&v.facts.boot_pid);
            kv(cv, x, y, b"boot_pid", b.get(), TEXT);
            y += lh;
            kv(
                cv,
                x,
                y,
                b"Boot clock",
                &v.facts.utc[..v.facts.utc_len],
                TEXT,
            );
            y += lh;
            b = Buf::new();
            b.s(v.fw_vendor)
                .s(b"  rev ")
                .dec((v.fw_rev >> 16) as u64)
                .s(b".")
                .dec((v.fw_rev & 0xFFFF) as u64);
            kv(cv, x, y, b"Firmware", b.get(), TEXT);
            y += lh;
            b = Buf::new();
            b.dec(v.gop_w as u64).s(b" x ").dec(v.gop_h as u64).s(b"  ");
            b.s(if v.gop_fmt == 0 { b"RGBx" } else { b"BGRx" })
                .s(b"  stride ")
                .dec(v.gop_pps as u64);
            kv(cv, x, y, b"Display (GOP)", b.get(), TEXT);
            y += lh;
            b = Buf::new();
            match v.mem {
                Some((free, total)) => {
                    b.dec(free >> 20)
                        .s(b" MiB free of ")
                        .dec(total >> 20)
                        .s(b" MiB (UEFI map)");
                }
                None => {
                    b.s(b"not read (memory map unavailable)");
                }
            }
            kv(cv, x, y, b"Memory", b.get(), TEXT);
            y += lh;
            b = Buf::new();
            b.dec(v.tsc_khz / 1000).s(b" MHz (TSC vs firmware Stall)");
            kv(cv, x, y, b"CPU clock", b.get(), TEXT);
            y += lh;
            b = Buf::new();
            hms(&mut b, v.uptime_s);
            b.s(b"  since the desktop started");
            kv(cv, x, y, b"Uptime", b.get(), TEXT);
            y += lh;
            b = Buf::new();
            b.dec(v.facts.pci_devices as u64).s(b" device-functions");
            kv(cv, x, y, b"PCI", b.get(), TEXT);
            y += lh;
            let wd = console::watchdog_text(v.wd);
            kv(
                cv,
                x,
                y,
                b"Watchdog",
                wd,
                if matches!(v.wd, Watchdog::Disabled | Watchdog::Unsupported) {
                    GREEN
                } else {
                    RED
                },
            );
            y += lh;
            kv(
                cv,
                x,
                y,
                b"Engines",
                b"E=0  fire=0  (nothing fires but what you click)",
                GREEN,
            );
        }
        App::Devices => draw_devices(cv, body, v, scroll),
        App::Clock => draw_clock(cv, body, v),
        App::Power => {
            cv.text(x, y, b"Leave the Asolaria desktop:", TEXT, 1);
            let (r1, r2, r3) = power_buttons(body);
            let c = v.power_confirm;
            button(
                cv,
                r1,
                if c == Some(0) {
                    b"Click again: RESTART"
                } else {
                    b"Restart  (R)"
                },
                if c == Some(0) { RED } else { AMBER },
            );
            button(
                cv,
                r2,
                if c == Some(2) {
                    b"Click again: POWER OFF"
                } else {
                    b"Power off  (P)"
                },
                RED,
            );
            button(cv, r3, b"Text console  (T)", CYAN);
            let note: &[u8] = if c.is_some() {
                b"Confirm with a second click or Enter. Esc cancels."
            } else {
                b"Restart / power off go through UEFI ResetSystem."
            };
            cv.text(x, body.y + body.h - 26, note, DIM, 1);
        }
        App::About => {
            iso_cube(
                cv,
                body.x + 86,
                body.y + 104,
                58,
                256,
                rgb(0x05, 0x10, 0x18),
            );
            let tx = body.x + 176;
            cv.text(tx, y + 6, b"ASOLARIA ASI OS", CYAN, 2);
            y += 50;
            let lines: [(&[u8], u32); 7] = [
                (b"federation-1024 kernel  .  0.2.0-phase3-scaffold", TEXT),
                (b"balanced ternary  .  27 = 3^3  .  integer only", TEXT),
                (b"UEFI desktop  .  Rust no_std  .  0 external crates", TEXT),
                (b"hot path: HBI -> HBP -> SHA -> SH -> HASH", DIM),
                (b"built by seat ACER-CLAUDE-FABLE5", DIM),
                (b"pid 8467a937cba309f7  .  owner OP-JESSE", DIM),
                (
                    b"E=0  .  fire=0  .  nothing fires but what you click",
                    GREEN,
                ),
            ];
            for (s, c) in lines {
                cv.text(tx, y, s, c, 1);
                y += lh;
            }
        }
        App::Help => {
            let lines: [(&[u8], u32); 14] = [
                (b"MOUSE / TOUCHPAD", CYAN),
                (
                    b"  double-click an icon to open it; drag a title bar to move",
                    TEXT,
                ),
                (
                    b"  click the taskbar to switch, minimise or restore windows",
                    TEXT,
                ),
                (b"  wheel scrolls the Console and Devices windows", TEXT),
                (
                    b"KEYBOARD (always works, even with no pointer driver)",
                    CYAN,
                ),
                (
                    b"  F1 help    F2 start menu    F3 next window    F4 close",
                    TEXT,
                ),
                (b"  F5 keyboard pointer: arrows move, Enter clicks,", TEXT),
                (
                    b"     Space holds the button (drag), Esc leaves the mode",
                    TEXT,
                ),
                (
                    b"  on the desktop: arrows / Tab pick an icon, Enter opens",
                    TEXT,
                ),
                (
                    b"  Console: type a command, Enter runs it, PgUp/PgDn scroll",
                    TEXT,
                ),
                (b"CONSOLE COMMANDS", CYAN),
                (b"  ping  vers  pid  info  devices  clear  help", TEXT),
                (b"  text (text console)   reboot   poweroff", TEXT),
                (b"POINTER DEVICES FOUND AT START", CYAN),
            ];
            for (s, c) in lines {
                cv.text(x, y, s, c, 1);
                y += lh;
            }
            let mut b = Buf::new();
            pointer_line(&mut b, v);
            cv.text(
                x,
                y,
                b.get(),
                if v.ptr_simple + v.ptr_abs > 0 {
                    GREEN
                } else {
                    AMBER
                },
                1,
            );
            if v.kbd_pointer {
                cv.text(
                    x,
                    y + lh,
                    b"  keyboard pointer is ON  (F5 or Esc leaves it)",
                    AMBER,
                    1,
                );
            }
        }
    }
}

/// "simple N  absolute M  (...)" — the pointer report used by Help, Devices and the tray.
pub(crate) fn pointer_line(b: &mut Buf, v: &View) {
    b.s(b"  ")
        .s(v.ptr_note)
        .s(b"  relative ")
        .dec(v.ptr_simple as u64);
    b.s(b"  absolute ").dec(v.ptr_abs as u64);
    b.s(b"  (fw handles ")
        .dec(v.ptr_handles.0 as u64)
        .s(b"/")
        .dec(v.ptr_handles.1 as u64);
    b.s(if v.ptr_splitter {
        b" via splitter"
    } else {
        b""
    });
    if v.usb_connected > 0 {
        b.s(b", usb hosts asked ").dec(v.usb_connected as u64);
    }
    b.s(b")");
    if v.ptr_simple + v.ptr_abs == 0 {
        b.s(b"  -> keyboard pointer F5");
    }
}

/// "Touchpad  I2C-HID ready  ctl 0:15.1 8086:a369 bar .. comp .. D3->D0" (probe outcome).
pub(crate) fn i2c_line1(b: &mut Buf, r: &crate::i2chid::Report) {
    b.s(b"Touchpad   I2C-HID ")
        .s(r.stage.text())
        .s(b"  controllers ")
        .dec(r.controllers as u64);
    if r.controllers > 0 {
        b.s(b"  last 0:")
            .hex(&[r.dev])
            .s(b".")
            .dec(r.fun as u64)
            .s(b" 8086:")
            .hex16(r.did)
            .s(b"  bar ")
            .hex(&r.bar.to_be_bytes())
            .s(b"  comp ")
            .hex(&r.comp_type.to_be_bytes())
            .s(b"  was D")
            .dec(r.pm_was as u64);
    }
}

/// "  addr 2c  hid 06cb:7db5  rdesc 693  max_in 30  mouse id 2  gpio ..." (device side).
pub(crate) fn i2c_line2(b: &mut Buf, r: &crate::i2chid::Report) {
    b.s(b"           addr ")
        .hex(&[r.addr])
        .s(b"  hid ")
        .hex16(r.vid)
        .s(b":")
        .hex16(r.pid)
        .s(b"  rdesc ")
        .dec(r.rdesc_len as u64)
        .s(b"  max_in ")
        .dec(r.max_in as u64)
        .s(b"  mouse_id ")
        .dec(r.mouse_id as u64)
        .s(b"  abort ")
        .hex(&r.last_abort.to_be_bytes())
        .s(b"  gpio padbar ")
        .hex(&r.gpio_padbar.to_be_bytes())
        .s(b" dw0 ")
        .hex(&r.gpio_dw0.to_be_bytes())
        .s(if r.gpio_saw_low {
            b" saw-low"
        } else {
            b" never-low"
        })
        .s(if r.gpio_used {
            b" irq-gated"
        } else {
            b" blind-poll"
        });
}

pub(crate) fn hms(b: &mut Buf, s: u64) {
    b.dec(s / 3600)
        .s(b":")
        .d2(((s / 60) % 60) as u8)
        .s(b":")
        .d2((s % 60) as u8);
}

/// The three Power buttons: (restart, power off, text console).
pub(crate) fn power_buttons(body: Rect) -> (Rect, Rect, Rect) {
    let w = (body.w - 64) / 3;
    let y = body.y + 64;
    (
        Rect::new(body.x + 16, y, w, 64),
        Rect::new(body.x + 32 + w, y, w, 64),
        Rect::new(body.x + 48 + 2 * w, y, w, 64),
    )
}

pub(crate) fn button(cv: &mut Canvas, r: Rect, label: &[u8], c: u32) {
    cv.round(r, 10, mix(PANEL2, c, 60), 256);
    cv.round(Rect::new(r.x, r.y, r.w, 3), 2, c, 256);
    // Split long labels on ": " into two lines.
    let (a, b) = match label.windows(2).position(|w| w == b": ") {
        Some(p) => (&label[..p + 1], &label[p + 2..]),
        None => (label, &label[..0]),
    };
    if b.is_empty() {
        cv.text_center((r.x, r.w), r.y + r.h / 2 - 8, a, TEXT, 1, false);
    } else {
        cv.text_center((r.x, r.w), r.y + r.h / 2 - 18, a, TEXT, 1, false);
        cv.text_center((r.x, r.w), r.y + r.h / 2 + 2, b, c, 1, false);
    }
}

fn draw_console(
    cv: &mut Canvas,
    body: Rect,
    t: &Term,
    v: &View,
    prompt: &[u8],
    status: Option<(&[u8], u32)>,
) {
    cv.fill(body, rgb(0x02, 0x04, 0x08));
    let lh = line_h(1) + 2;
    let x = body.x + 10;
    // Visible text rows (the input line takes the last one).
    let vis = ((body.h - 16) / lh).max(2) as usize;
    let hist = vis - 1;
    // Completed lines exclude the empty line under the cursor if it is blank.
    let last = t.used;
    let end = last.saturating_sub(t.scroll);
    let start = end.saturating_sub(hist);
    let mut y = body.y + 8;
    for i in start..end {
        let (cells, tones) = t.line(i);
        let mut n = cells.len();
        while n > 0 && cells[n - 1] == b' ' {
            n -= 1;
        }
        let maxc = ((body.w - 20) / 8).max(0) as usize;
        let n = n.min(maxc);
        // Draw runs of equal tone.
        let mut a = 0;
        while a < n {
            let mut b = a + 1;
            while b < n && tones[b] == tones[a] {
                b += 1;
            }
            cv.text(x + a as i32 * 8, y, &cells[a..b], tone_color(tones[a]), 1);
            a = b;
        }
        y += lh;
    }
    // Input line pinned at the bottom.
    let iy = body.y + body.h - lh - 6;
    cv.fill(Rect::new(body.x, iy - 4, body.w, 1), rgb(0x16, 0x22, 0x38));
    let px = cv.text(x, iy, prompt, AMBER, 1);
    let ex = cv.text(px, iy, &t.input[..t.ilen], WHITE, 1);
    if v.caret && v.focused {
        cv.fill(Rect::new(ex + 1, iy + 2, 7, 13), CYAN);
    }
    if let Some((s, c)) = status {
        let w = text_w(s, 1);
        cv.text(body.x + body.w - w - 12, iy, s, c, 1);
    }
    if t.scroll > 0 {
        let mut b = Buf::new();
        b.s(b"scrollback -").dec(t.scroll as u64).s(b" (PgDn)");
        let w = text_w(b.get(), 1);
        cv.text(body.x + body.w - w - 12, body.y + 6, b.get(), AMBER, 1);
    }
}

fn draw_devices(cv: &mut Canvas, body: Rect, v: &View, scroll: i32) {
    let x = body.x + 14;
    let mut y = body.y + 12;
    let lh = line_h(1) + 3;
    cv.text(x, y, b"DRIVERS IN USE", CYAN, 1);
    y += lh + 2;
    let mut b = Buf::new();
    b.s(b"Display    GOP framebuffer ")
        .dec(v.gop_w as u64)
        .s(b"x")
        .dec(v.gop_h as u64);
    b.s(if v.gop_fmt == 0 { b" RGBx" } else { b" BGRx" })
        .s(b" -> desktop back buffer");
    cv.text(x, y, b.get(), TEXT, 1);
    y += lh;
    cv.text(x, y, b"Keyboard   UEFI Simple Text Input (ConIn)", TEXT, 1);
    y += lh;
    b = Buf::new();
    b.s(b"Pointer  ");
    pointer_line(&mut b, v);
    cv.text(
        x,
        y,
        b.get(),
        if v.ptr_simple + v.ptr_abs > 0 {
            TEXT
        } else {
            AMBER
        },
        1,
    );
    y += lh;
    if let Some(r) = v.i2c {
        let ok = r.stage == crate::i2chid::Stage::Ready;
        let c = if ok { GREEN } else { AMBER };
        b = Buf::new();
        i2c_line1(&mut b, &r);
        cv.text(x, y, b.get(), c, 1);
        y += lh;
        b = Buf::new();
        i2c_line2(&mut b, &r);
        cv.text(x, y, b.get(), c, 1);
        y += lh;
    }
    let st: &[u8] = if v.facts.rst_vmd {
        b"Storage    Intel RST/VMD 8086:282A gate MATCHED (scaffold: VMD walk is the next step)"
    } else {
        b"Storage    RST/VMD gate: 8086:282A not on this machine"
    };
    cv.text(x, y, st, if v.facts.rst_vmd { AMBER } else { DIM }, 1);
    y += lh + 6;
    cv.fill(Rect::new(body.x + 8, y - 4, body.w - 16, 1), LINE);
    cv.text(
        x,
        y,
        b"BUS:DV.F  VEN:DEV    CLASS  TYPE                          VENDOR",
        DIM,
        1,
    );
    let head_y = y;
    y += lh;
    let n = (v.facts.pci_devices as usize).min(v.facts.pci.len());
    let first = (scroll.max(0) as usize).min(n.saturating_sub(1));
    let old = cv.set_clip(Rect::new(body.x, y, body.w, body.y + body.h - y - 4));
    for (k, d) in v.facts.pci[first..n].iter().enumerate() {
        let ry = y + k as i32 * lh;
        if ry > body.y + body.h {
            break;
        }
        if k % 2 == 0 {
            cv.fill(
                Rect::new(body.x + 8, ry - 2, body.w - 16, lh),
                rgb(0x10, 0x18, 0x2a),
            );
        }
        let mut b = Buf::new();
        b.hex(&[d.bus])
            .s(b":")
            .hex(&[d.dev])
            .s(b".")
            .dec(d.func as u64)
            .pad(10);
        b.hex16(d.vendor).s(b":").hex16(d.device).pad(21);
        b.hex(&[d.class]).s(b":").hex(&[d.subclass]).pad(28);
        b.s(class_name(d.class, d.subclass))
            .pad(58)
            .s(vendor_name(d.vendor));
        let c = if d.vendor == 0x8086 && d.device == 0x282A {
            AMBER
        } else {
            TEXT
        };
        cv.text(x, ry, b.get(), c, 1);
    }
    cv.restore_clip(old);
    if n > 0 {
        let mut b = Buf::new();
        b.dec(first as u64 + 1)
            .s(b"-")
            .dec(n as u64)
            .s(b" of ")
            .dec(n as u64);
        let w = text_w(b.get(), 1);
        cv.text(body.x + body.w - w - 12, head_y, b.get(), DIM, 1);
    }
}

fn draw_clock(cv: &mut Canvas, body: Rect, v: &View) {
    let cx = body.x + body.w / 2;
    let r = ((body.h - 96) / 2).min(body.w / 2 - 20).max(30);
    let cy = body.y + 16 + r;
    cv.disc(cx, cy, r, rgb(0x10, 0x18, 0x2c));
    cv.ring(cx, cy, r, 3, AMBER);
    for m in 0..60 {
        let (ri, c) = if m % 5 == 0 {
            (r - 12, TEXT)
        } else {
            (r - 6, DIM)
        };
        let (s, co) = (sin60(m), cos60(m));
        cv.line(
            cx + s * ri / 1000,
            cy - co * ri / 1000,
            cx + s * (r - 3) / 1000,
            cy - co * (r - 3) / 1000,
            c,
        );
    }
    let t = v.now;
    let hpos = (t.hour as i32 % 12) * 5 + t.min as i32 / 12;
    let hand = |cv: &mut Canvas, pos: i32, len: i32, w: i32, c: u32| {
        cv.line_w(
            cx,
            cy,
            cx + sin60(pos) * len / 1000,
            cy - cos60(pos) * len / 1000,
            w,
            c,
        );
    };
    hand(cv, hpos, r * 50 / 100, 4, TEXT);
    hand(cv, t.min as i32, r * 75 / 100, 3, TEXT);
    hand(cv, t.sec as i32, r * 85 / 100, 1, RED);
    cv.disc(cx, cy, 4, AMBER);
    let mut b = Buf::new();
    b.d2(t.hour).s(b":").d2(t.min).s(b":").d2(t.sec);
    cv.text_center((body.x, body.w), cy + r + 12, b.get(), CYAN, 2, false);
    let mut d = Buf::new();
    d.dec(t.year as u64)
        .s(b"-")
        .d2(t.month)
        .s(b"-")
        .d2(t.day)
        .s(b"   CMOS real-time clock");
    cv.text_center((body.x, body.w), cy + r + 48, d.get(), DIM, 1, false);
}

// ------------------------------------------------------------------ name tables
pub(crate) fn vendor_name(v: u16) -> &'static [u8] {
    match v {
        0x8086 => b"Intel",
        0x1022 => b"AMD",
        0x1002 => b"AMD/ATI",
        0x10de => b"NVIDIA",
        0x10ec => b"Realtek",
        0x168c => b"Qualcomm Atheros",
        0x14e4 => b"Broadcom",
        0x1b36 => b"Red Hat (QEMU)",
        0x1af4 => b"virtio",
        0x1234 => b"QEMU/Bochs",
        0x15ad => b"VMware",
        0x144d => b"Samsung",
        0x15b7 => b"SanDisk/WD",
        0x1c5c => b"SK hynix",
        0x8087 => b"Intel (wireless)",
        0x1217 => b"O2 Micro",
        0x10b5 => b"PLX",
        _ => b"-",
    }
}

pub(crate) fn class_name(class: u8, sub: u8) -> &'static [u8] {
    match (class, sub) {
        (0x00, _) => b"unclassified",
        (0x01, 0x01) => b"IDE storage",
        (0x01, 0x04) => b"RAID storage",
        (0x01, 0x06) => b"SATA (AHCI) storage",
        (0x01, 0x08) => b"NVMe storage",
        (0x01, _) => b"mass storage",
        (0x02, 0x00) => b"Ethernet",
        (0x02, 0x80) => b"network (wireless)",
        (0x02, _) => b"network",
        (0x03, 0x00) => b"VGA display",
        (0x03, 0x02) => b"3D display (GPU)",
        (0x03, _) => b"display",
        (0x04, 0x03) => b"audio (HD)",
        (0x04, _) => b"multimedia",
        (0x05, _) => b"memory controller",
        (0x06, 0x00) => b"host bridge",
        (0x06, 0x01) => b"ISA/LPC bridge",
        (0x06, 0x04) => b"PCI bridge",
        (0x06, _) => b"bridge",
        (0x07, _) => b"communication",
        (0x08, _) => b"system peripheral",
        (0x09, _) => b"input device",
        (0x0c, 0x03) => b"USB controller",
        (0x0c, 0x05) => b"SMBus",
        (0x0c, 0x80) => b"serial bus (other)",
        (0x0c, _) => b"serial bus",
        (0x0d, _) => b"wireless",
        (0x10, _) => b"encryption",
        (0x11, _) => b"signal processing",
        _ => b"other",
    }
}
