//! Console core · the command set shared by the text console and the desktop's Console window.
//!
//! History: before 2026-10-09 the boot path painted the splash and handed off to `init::run()`,
//! an envelope loop with no input source and no screen output, and the UEFI watchdog (armed by
//! the boot manager, 5 minutes) reset the machine — "hangs on the logo, then back to Windows".
//! This module:
//!   1. disables the UEFI watchdog (`SetWatchdogTimer(0, 0, 0, null)`), status shown as returned,
//!   2. keeps the splash up briefly, then starts the graphical desktop (`gui`) when the firmware
//!      gives a usable framebuffer, else the firmware text console (ConOut),
//!   3. turns each command line into a real init envelope (`PING`/`VERS`/`PIDA`) driven through
//!      `init::exchange` — the SAME [`execute`] serves the text console and the Console window,
//!      writing to whatever [`Sink`] the caller supplies,
//!   4. `reboot` / `poweroff` go through UEFI `ResetSystem` (never a silent spin).
//!
//! Integer only. No heap. E=0: nothing here fires an engine.

use crate::hwinv::PciDev;
use crate::init;
use crate::{EfiSystemTable, EfiTableHeader};
use asolaria_kernel_core::envelope::ENVELOPE_SLOT_BYTES;
use core::sync::atomic::{AtomicUsize, Ordering};

/// UEFI status EFI_UNSUPPORTED (high bit | 3): firmware has no watchdog to disable.
const EFI_UNSUPPORTED: usize = (1usize << 63) | 3;

#[repr(C)]
pub(crate) struct InputKey {
    pub scan_code: u16,
    pub unicode_char: u16,
}

#[repr(C)]
pub(crate) struct SimpleTextInput {
    reset: unsafe extern "efiapi" fn(*mut SimpleTextInput, bool) -> usize,
    read_key_stroke: unsafe extern "efiapi" fn(*mut SimpleTextInput, *mut InputKey) -> usize,
}

#[repr(C)]
pub(crate) struct TextOut {
    reset: usize,
    output_string: unsafe extern "efiapi" fn(*mut TextOut, *const u16) -> usize,
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: unsafe extern "efiapi" fn(*mut TextOut, usize) -> usize,
    clear_screen: unsafe extern "efiapi" fn(*mut TextOut) -> usize,
    set_cursor_position: usize,
    enable_cursor: unsafe extern "efiapi" fn(*mut TextOut, bool) -> usize,
}

/// Boot services, indexed per UEFI 2.x: Stall = 28, SetWatchdogTimer = 29.
#[repr(C)]
pub(crate) struct BootSvc {
    hdr: EfiTableHeader,
    _pad: [usize; 28],
    stall: unsafe extern "efiapi" fn(usize) -> usize,
    set_watchdog_timer: unsafe extern "efiapi" fn(usize, u64, usize, *const u16) -> usize,
}

/// Runtime services, ResetSystem = index 10.
#[repr(C)]
pub(crate) struct RuntimeSvc {
    hdr: EfiTableHeader,
    _pad: [usize; 10],
    // Spec: ResetSystem does not return. Declared `-> usize` (not `!`) so firmware that DOES
    // return is not undefined behaviour; the caller reports it and carries on.
    reset_system: unsafe extern "efiapi" fn(u32, usize, usize, *const u16) -> usize,
}

/// Facts gathered by the boot stages, shown on screen.
pub(crate) struct BootFacts {
    pub device_digest: [u8; 32],
    pub boot_pid: [u8; 8],
    pub utc: [u8; 20],
    pub utc_len: usize,
    pub pci_devices: u32,
    pub rst_vmd: bool,
    pub pci: [PciDev; crate::hwinv::DEVICE_CAP as usize],
}

/// Outcome of the watchdog-disable call, shown as returned (never assumed).
#[derive(Clone, Copy)]
pub(crate) enum Watchdog {
    Disabled,
    Unsupported,
    Failed,
    NoBootServices,
}

pub(crate) fn watchdog_text(wd: Watchdog) -> &'static [u8] {
    match wd {
        Watchdog::Disabled => b"disabled",
        Watchdog::Unsupported => b"none on this firmware",
        Watchdog::Failed => b"DISABLE FAILED (firmware may reset in 5 min)",
        Watchdog::NoBootServices => b"unknown (no boot services)",
    }
}

/// Runtime services, published for the panic handler: with the watchdog disabled a panic
/// would otherwise hang until power-off, so the handler cold-resets instead.
static PANIC_RT: AtomicUsize = AtomicUsize::new(0);

/// Cold-reset via UEFI ResetSystem if the console has published runtime services. Returns
/// only if the firmware ignores the reset (or the console never started).
pub(crate) unsafe fn panic_reset() {
    let rt = PANIC_RT.load(Ordering::Relaxed) as *mut RuntimeSvc;
    if !rt.is_null() {
        let _ = ((*rt).reset_system)(0, 0, 0, core::ptr::null());
    }
}

// ------------------------------------------------------------------ output sinks

/// Colour role of console output; each front-end maps it to its own palette.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Normal,
    Accent,
    Warn,
    Error,
}

/// Where command output goes: the firmware text console or a desktop window.
pub(crate) trait Sink {
    /// Printable ASCII and '\n' only (callers sanitise untrusted bytes with [`text`]).
    fn put(&mut self, s: &[u8]);
    fn tone(&mut self, t: Tone);
}

/// Print untrusted bytes: anything outside printable ASCII becomes '.', so a NUL cannot cut the
/// line short and control bytes never reach the screen.
pub(crate) fn text(o: &mut dyn Sink, s: &[u8]) {
    let mut b = [0u8; 64];
    for chunk in s.chunks(b.len()) {
        for (d, &x) in b.iter_mut().zip(chunk) {
            *d = if (0x20..0x7F).contains(&x) { x } else { b'.' };
        }
        o.put(&b[..chunk.len()]);
    }
}

pub(crate) fn hex(o: &mut dyn Sink, bytes: &[u8]) {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut b = [0u8; 64];
    let mut n = 0;
    for &x in bytes.iter().take(32) {
        b[n] = H[(x >> 4) as usize];
        b[n + 1] = H[(x & 15) as usize];
        n += 2;
    }
    o.put(&b[..n]);
}

pub(crate) fn dec(o: &mut dyn Sink, v: u64) {
    let mut b = [0u8; 20];
    let n = fmt_dec(&mut b, v);
    o.put(&b[20 - n..]);
}

/// Right-aligned decimal into `b` (20 bytes); returns the digit count.
pub(crate) fn fmt_dec(b: &mut [u8; 20], mut v: u64) -> usize {
    let mut n = 0;
    loop {
        n += 1;
        b[20 - n] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    n
}

// ------------------------------------------------------------------ shared context

/// Everything the front-ends share: firmware handles, boot facts, the watchdog outcome and the
/// envelope sequence counter.
pub(crate) struct Ctx<'a> {
    pub st: *mut EfiSystemTable,
    out: *mut TextOut,
    inp: *mut SimpleTextInput,
    bs: *mut BootSvc,
    rt: *mut RuntimeSvc,
    pub facts: &'a BootFacts,
    pub wd: Watchdog,
    pub seq: u64,
    /// Set by the desktop once it has started, so the text console can offer `desktop`.
    pub gui_ok: bool,
    /// Pointer report from the desktop, for `info` in either front-end.
    pub pointer_simple: usize,
    pub pointer_abs: usize,
}

impl Ctx<'_> {
    pub(crate) unsafe fn stall_us(&self, us: usize) {
        if !self.bs.is_null() {
            ((*self.bs).stall)(us);
        }
    }

    /// Non-blocking key read: Some(key) / None.
    pub(crate) unsafe fn key(&self) -> Option<InputKey> {
        if self.inp.is_null() {
            return None;
        }
        let mut k = InputKey {
            scan_code: 0,
            unicode_char: 0,
        };
        // 0 = EFI_SUCCESS; anything else (EFI_NOT_READY when no key is waiting) = no key.
        let st = ((*self.inp).read_key_stroke)(self.inp, &mut k);
        (st == 0).then_some(k)
    }

    /// Show or hide the firmware text cursor (hidden while the desktop owns the screen).
    pub(crate) unsafe fn text_cursor(&self, on: bool) {
        if !self.out.is_null() {
            ((*self.out).enable_cursor)(self.out, on);
        }
    }

    /// UEFI ResetSystem: kind 0 = cold reset, 2 = shutdown. Returns only if firmware ignored it.
    pub(crate) unsafe fn reset(&self, kind: u32) {
        let _ = asolaria_kernel_core::syscall::sys_cosign_append(b"console-reset-request");
        crate::serial_print(b"  CONSOLE|reset=requested|e=0|json=0\r\n");
        self.stall_us(300_000);
        if !self.rt.is_null() {
            let _ = ((*self.rt).reset_system)(kind, 0, 0, core::ptr::null());
        }
    }
}

/// What the caller must do after a command.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    None,
    Clear,
    Reset(u32),
    Desktop,
    TextMode,
}

pub(crate) fn help(o: &mut dyn Sink) {
    o.put(b"  ping      envelope PING  -> init -> PONG\n");
    o.put(b"  vers      envelope VERS  -> kernel version\n");
    o.put(b"  pid       envelope PIDA  -> federation anchor PID\n");
    o.put(b"  info      boot facts (device, boot pid, clock, pci)\n");
    o.put(b"  devices   PCI inventory (read-only)\n");
    o.put(b"  clear     clear the screen\n");
    o.put(b"  desktop   graphical desktop     text   text console\n");
    o.put(b"  reboot    UEFI cold reset       poweroff  UEFI shutdown\n");
}

/// The boot-facts block (`info`).
pub(crate) fn info(o: &mut dyn Sink, c: &Ctx) {
    let f = c.facts;
    o.tone(Tone::Normal);
    o.put(b"  device_pid  ");
    hex(o, &f.device_digest[..8]);
    o.put(b"\n  boot_pid    ");
    hex(o, &f.boot_pid);
    o.put(b"\n  boot_clock  ");
    o.put(&f.utc[..f.utc_len]);
    o.put(b"  (CMOS RTC)\n  pci         ");
    dec(o, f.pci_devices as u64);
    if f.pci_devices >= crate::hwinv::DEVICE_CAP {
        o.put(b"+ (capped)");
    }
    o.put(b" device-functions   rst/vmd ");
    o.put(if f.rst_vmd { b"MATCHED" } else { b"n/a" });
    o.put(b"\n  watchdog    ");
    o.put(watchdog_text(c.wd));
    o.put(b"\n  pointer     simple=");
    dec(o, c.pointer_simple as u64);
    o.put(b" absolute=");
    dec(o, c.pointer_abs as u64);
    o.put(b"   E=0   fire=0\n");
}

/// One PCI row, `bb:dd.f  vvvv:dddd  cc:ss  <class name>`.
pub(crate) fn devices(o: &mut dyn Sink, f: &BootFacts) {
    let n = (f.pci_devices as usize).min(f.pci.len());
    for d in &f.pci[..n] {
        let mut b = [0u8; 40];
        let mut i = crate::hwinv::put_bytes(&mut b, 0, b"  ");
        i = crate::hwinv::put_hex8(&mut b, i, d.bus);
        b[i] = b':';
        i = crate::hwinv::put_hex8(&mut b, i + 1, d.dev);
        b[i] = b'.';
        b[i + 1] = b'0' + d.func;
        i = crate::hwinv::put_bytes(&mut b, i + 2, b"  ");
        i = crate::hwinv::put_hex8(&mut b, i, (d.vendor >> 8) as u8);
        i = crate::hwinv::put_hex8(&mut b, i, d.vendor as u8);
        b[i] = b':';
        i = crate::hwinv::put_hex8(&mut b, i + 1, (d.device >> 8) as u8);
        i = crate::hwinv::put_hex8(&mut b, i, d.device as u8);
        i = crate::hwinv::put_bytes(&mut b, i, b"  ");
        o.put(&b[..i]);
        o.put(crate::apps::class_name(d.class, d.subclass));
        o.put(b"\n");
    }
}

/// Send one envelope type through the real init dispatch and print the reply.
fn envelope(o: &mut dyn Sink, ty: &[u8; 4], seq: &mut u64) {
    // As large as the bus slot itself, so a reply can never be left stuck in it.
    let mut reply = [0u8; ENVELOPE_SLOT_BYTES];
    match init::exchange(ty, seq, &mut reply) {
        Ok(n) => {
            o.tone(Tone::Accent);
            text(o, &reply[..4]);
            o.tone(Tone::Normal);
            // payload starts after the 12-byte header; trim zero padding
            let mut end = n;
            while end > 12 && reply[end - 1] == 0 {
                end -= 1;
            }
            if end > 12 {
                o.put(b"  ");
                text(o, &reply[12..end]);
            }
            o.put(b"\n");
        }
        Err(e) => {
            o.tone(Tone::Error);
            o.put(b"  envelope bus error: ");
            o.put(e);
            o.put(b"\n");
            o.tone(Tone::Normal);
        }
    }
}

/// Run one command line. Output goes to `o`; the returned [`Action`] is the caller's to carry
/// out (clearing its own screen, switching front-end, or resetting through [`Ctx::reset`]).
pub(crate) fn execute(cmd: &[u8], o: &mut dyn Sink, c: &mut Ctx) -> Action {
    let cmd = trim(cmd);
    match cmd {
        b"" => Action::None,
        b"help" | b"?" => {
            help(o);
            Action::None
        }
        b"ping" => {
            envelope(o, b"PING", &mut c.seq);
            Action::None
        }
        b"vers" | b"version" => {
            envelope(o, b"VERS", &mut c.seq);
            Action::None
        }
        b"pid" => {
            envelope(o, b"PIDA", &mut c.seq);
            Action::None
        }
        b"info" => {
            info(o, c);
            Action::None
        }
        b"devices" | b"lspci" => {
            devices(o, c.facts);
            Action::None
        }
        b"clear" | b"cls" => Action::Clear,
        b"desktop" | b"gui" => Action::Desktop,
        b"text" => Action::TextMode,
        b"reboot" | b"restart" => {
            o.put(b"  resetting via UEFI ResetSystem\n");
            Action::Reset(0)
        }
        b"poweroff" | b"shutdown" => {
            o.put(b"  powering off via UEFI ResetSystem\n");
            Action::Reset(2)
        }
        _ => {
            o.tone(Tone::Error);
            o.put(b"  unknown command: ");
            text(o, cmd);
            o.put(b"   (type 'help')\n");
            o.tone(Tone::Normal);
            Action::None
        }
    }
}

fn trim(s: &[u8]) -> &[u8] {
    let mut a = 0;
    let mut b = s.len();
    while a < b && s[a] == b' ' {
        a += 1;
    }
    while b > a && s[b - 1] == b' ' {
        b -= 1;
    }
    &s[a..b]
}

// ------------------------------------------------------------------ text console front-end

// EFI text attributes: foreground | background<<4.
const CYAN: usize = 0x03 | 0x08; // light cyan
const AMBER: usize = 0x0E; // yellow
const GREY: usize = 0x07;
const RED: usize = 0x0C;

/// The firmware text console as a [`Sink`].
struct TextSink {
    out: *mut TextOut,
}

impl Sink for TextSink {
    fn put(&mut self, s: &[u8]) {
        if self.out.is_null() {
            return;
        }
        // Chunk through a fixed UTF-16 buffer; translate '\n' to "\r\n".
        let mut buf = [0u16; 130];
        let mut i = 0;
        for &b in s {
            if b == b'\n' {
                buf[i] = b'\r' as u16;
                i += 1;
            }
            buf[i] = b as u16;
            i += 1;
            if i >= 126 {
                buf[i] = 0;
                // SAFETY: `out` is the firmware ConOut, valid while boot services are active.
                unsafe { ((*self.out).output_string)(self.out, buf.as_ptr()) };
                i = 0;
            }
        }
        buf[i] = 0;
        // SAFETY: as above.
        unsafe { ((*self.out).output_string)(self.out, buf.as_ptr()) };
    }

    fn tone(&mut self, t: Tone) {
        let attr = match t {
            Tone::Normal => GREY,
            Tone::Accent => CYAN,
            Tone::Warn => AMBER,
            Tone::Error => RED,
        };
        if !self.out.is_null() {
            // SAFETY: firmware ConOut.
            unsafe { ((*self.out).set_attribute)(self.out, attr) };
        }
    }
}

unsafe fn banner(o: &mut TextSink, c: &Ctx) {
    if !o.out.is_null() {
        ((*o.out).clear_screen)(o.out);
        ((*o.out).enable_cursor)(o.out, true);
    }
    o.tone(Tone::Accent);
    o.put(b"  ASOLARIA ASI OS  .  kernel ");
    o.put(asolaria_kernel_core::KERNEL_VERSION.as_bytes());
    o.put(b"  .  federation-1024  .  text console\n");
    o.tone(Tone::Warn);
    o.put(b"  ----------------------------------------------------------------\n");
    info(o, c);
    o.tone(Tone::Warn);
    o.put(b"  ----------------------------------------------------------------\n");
    o.tone(Tone::Normal);
    if c.gui_ok {
        o.put(b"  type 'help' for commands, 'desktop' for the graphical desktop\n\n");
    } else {
        o.put(b"  type 'help' for commands\n\n");
    }
}

/// The firmware text console. Returns when the user asks for the desktop and it is available.
pub(crate) unsafe fn text_console(c: &mut Ctx) {
    let mut o = TextSink { out: c.out };
    banner(&mut o, c);
    let mut line = [0u8; 64];
    let mut len = 0usize;
    let mut last_cr = false;
    o.tone(Tone::Warn);
    o.put(b"asolaria> ");
    o.tone(Tone::Normal);
    loop {
        let Some(k) = c.key() else {
            // Belt and braces: a mouse byte left in the i8042 output buffer would block every
            // keyboard byte behind it (the firmware keyboard driver never reads aux bytes).
            let _ = crate::ps2mouse::drain_aux();
            c.stall_us(10_000);
            continue;
        };
        let ch = k.unicode_char;
        // Enter is CR on a keyboard; some serial/remote terminals send LF or CR LF.
        let lf_after_cr = ch == 0x0A && last_cr;
        last_cr = ch == 0x0D;
        if lf_after_cr {
            continue;
        }
        if ch == 0x0D || ch == 0x0A {
            o.put(b"\n");
            let mut cmd = [0u8; 64];
            cmd[..len].copy_from_slice(&line[..len]);
            let n = len;
            len = 0;
            match execute(&cmd[..n], &mut o, c) {
                Action::Clear => banner(&mut o, c),
                Action::Desktop if c.gui_ok => return,
                Action::Desktop => o.put(b"  no usable framebuffer: the desktop is unavailable\n"),
                Action::Reset(kind) => {
                    c.reset(kind);
                    o.tone(Tone::Error);
                    o.put(b"  reset did not happen\n");
                    o.tone(Tone::Normal);
                }
                Action::TextMode | Action::None => {}
            }
            o.tone(Tone::Warn);
            o.put(b"asolaria> ");
            o.tone(Tone::Normal);
        } else if ch == 0x08 {
            if len > 0 {
                len -= 1;
                o.put(b"\x08 \x08");
            }
        } else if (0x20..0x7F).contains(&ch) && len < line.len() {
            let b = ch as u8;
            line[len] = b;
            len += 1;
            o.put(&[b]);
        }
    }
}

/// Run the console. Never returns.
pub(crate) unsafe fn run(st: *mut EfiSystemTable, facts: &BootFacts) -> ! {
    let mut c = Ctx {
        st,
        out: (*st).con_out as *mut TextOut,
        inp: (*st).con_in as *mut SimpleTextInput,
        bs: (*st).boot_services as *mut BootSvc,
        rt: (*st).runtime_services as *mut RuntimeSvc,
        facts,
        wd: Watchdog::NoBootServices,
        seq: 1,
        gui_ok: false,
        pointer_simple: 0,
        pointer_abs: 0,
    };
    PANIC_RT.store(c.rt as usize, Ordering::Relaxed);
    // 1. The watchdog: without this the firmware resets the machine 5 minutes after launch.
    //    The firmware's status is reported as returned, not assumed.
    c.wd = if c.bs.is_null() {
        Watchdog::NoBootServices
    } else {
        match ((*c.bs).set_watchdog_timer)(0, 0, 0, core::ptr::null()) {
            0 => Watchdog::Disabled,
            EFI_UNSUPPORTED => Watchdog::Unsupported,
            _ => Watchdog::Failed,
        }
    };
    crate::serial_print(b"  CONSOLE|watchdog=");
    crate::serial_print(watchdog_text(c.wd));
    crate::serial_print(b"|input=uefi_conin|e=0|json=0\r\n");
    // 2. Splash stays up 1.5 s, then the desktop (or the text console) takes the screen.
    c.stall_us(1_500_000);
    if !c.inp.is_null() {
        ((*c.inp).reset)(c.inp, false);
    }
    // Start from an empty bus.
    init::drain();
    // The desktop runs until reset (it opens the text console itself when asked). It returns
    // only when the firmware gives no usable framebuffer; then the text console is the OS.
    crate::gui::run(&mut c);
    loop {
        text_console(&mut c);
    }
}
