//! On-screen console · the fix for "boots, then sits on the loading screen forever".
//!
//! Before this module the boot path painted the ASOLARIA ASI OS splash and handed off to
//! `init::run()`, an envelope loop with no input source and no screen output: the machine was
//! running, but nothing could ever reach it and nothing was ever drawn again. On real firmware
//! the UEFI watchdog (armed by the boot manager, 5 minutes) then reset the machine, which reads
//! as "hangs on the logo, then falls back to Windows".
//!
//! This console:
//!   1. disables the UEFI watchdog (`SetWatchdogTimer(0, 0, 0, null)`),
//!   2. keeps the splash up briefly, then clears to the firmware text console (ConOut),
//!   3. shows the boot facts (device_pid, boot_pid, UTC, PCI count, storage driver gate),
//!   4. reads the keyboard through UEFI ConIn and turns each command line into a real
//!      init envelope (`PING`/`VERS`/`PIDA`/...), driven through `init::exchange`, and prints the
//!      reply — so the envelope-REPL is reachable from a keyboard,
//!   5. `reboot` / `poweroff` go through UEFI `ResetSystem` (never a silent spin).
//!
//! Integer only. No allocation. E=0: nothing here fires an engine.

use crate::init;
use crate::{EfiSystemTable, EfiTableHeader};

/// UEFI status EFI_UNSUPPORTED (high bit | 3): firmware has no watchdog to disable.
const EFI_UNSUPPORTED: usize = (1usize << 63) | 3;

#[repr(C)]
pub(crate) struct InputKey {
    scan_code: u16,
    unicode_char: u16,
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
}

/// Outcome of the watchdog-disable call, shown as returned (never assumed).
#[derive(Clone, Copy)]
enum Watchdog {
    Disabled,
    Unsupported,
    Failed,
    NoBootServices,
}

fn watchdog_text(wd: Watchdog) -> &'static [u8] {
    match wd {
        Watchdog::Disabled => b"disabled",
        Watchdog::Unsupported => b"none on this firmware",
        Watchdog::Failed => b"DISABLE FAILED (firmware may reset in 5 min)",
        Watchdog::NoBootServices => b"unknown (no boot services)",
    }
}

struct Con {
    out: *mut TextOut,
    inp: *mut SimpleTextInput,
    bs: *mut BootSvc,
    rt: *mut RuntimeSvc,
}

impl Con {
    unsafe fn print(&self, s: &[u8]) {
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
                ((*self.out).output_string)(self.out, buf.as_ptr());
                i = 0;
            }
        }
        buf[i] = 0;
        ((*self.out).output_string)(self.out, buf.as_ptr());
    }

    unsafe fn hex(&self, bytes: &[u8]) {
        const H: &[u8; 16] = b"0123456789abcdef";
        let mut b = [0u8; 64];
        let mut n = 0;
        for &x in bytes.iter().take(32) {
            b[n] = H[(x >> 4) as usize];
            b[n + 1] = H[(x & 15) as usize];
            n += 2;
        }
        self.print(&b[..n]);
    }

    unsafe fn dec(&self, mut v: u64) {
        let mut b = [0u8; 20];
        let mut n = b.len();
        loop {
            n -= 1;
            b[n] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.print(&b[n..]);
    }

    unsafe fn color(&self, attr: usize) {
        if !self.out.is_null() {
            ((*self.out).set_attribute)(self.out, attr);
        }
    }

    unsafe fn stall_us(&self, us: usize) {
        if !self.bs.is_null() {
            ((*self.bs).stall)(us);
        }
    }

    /// Non-blocking key read: Some(char or control) / None.
    unsafe fn key(&self) -> Option<InputKey> {
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
}

// EFI text attributes: foreground | background<<4.
const CYAN: usize = 0x03 | 0x08; // light cyan
const AMBER: usize = 0x0E; // yellow
const GREY: usize = 0x07;
const RED: usize = 0x0C;

unsafe fn banner(c: &Con, f: &BootFacts, wd: Watchdog) {
    if !c.out.is_null() {
        ((*c.out).clear_screen)(c.out);
        ((*c.out).enable_cursor)(c.out, true);
    }
    c.color(CYAN);
    c.print(b"  ASOLARIA ASI OS  .  kernel ");
    c.print(asolaria_kernel_core::KERNEL_VERSION.as_bytes());
    c.print(b"  .  federation-1024\n");
    c.color(AMBER);
    c.print(b"  ----------------------------------------------------------------\n");
    c.color(GREY);
    c.print(b"  device_pid  ");
    c.hex(&f.device_digest[..8]);
    c.print(b"\n  boot_pid    ");
    c.hex(&f.boot_pid);
    c.print(b"\n  boot_utc    ");
    c.print(&f.utc[..f.utc_len]);
    c.print(b"\n  pci         ");
    c.dec(f.pci_devices as u64);
    c.print(b" device-functions   storage-driver(rst/vmd) ");
    c.print(if f.rst_vmd { b"MATCHED" } else { b"n/a    " });
    c.print(b"\n  watchdog    ");
    c.print(watchdog_text(wd));
    c.print(b"   E=0   fire=0\n");
    c.color(AMBER);
    c.print(b"  ----------------------------------------------------------------\n");
    c.color(GREY);
    c.print(b"  type 'help' for commands\n\n");
}

unsafe fn help(c: &Con) {
    c.print(b"  ping      envelope PING  -> init -> PONG\n");
    c.print(b"  vers      envelope VERS  -> kernel version\n");
    c.print(b"  pid       envelope PIDA  -> federation anchor PID\n");
    c.print(b"  info      boot facts (device, boot pid, clock, pci)\n");
    c.print(b"  clear     redraw the screen\n");
    c.print(b"  reboot    UEFI cold reset\n");
    c.print(b"  poweroff  UEFI shutdown\n");
}

/// Send one envelope type through the real init dispatch and print the reply.
unsafe fn envelope(c: &Con, ty: &[u8; 4], seq: &mut u64) {
    // As large as init's biggest envelope, so a reply can never be left stuck in the bus slot.
    let mut reply = [0u8; 512];
    match init::exchange(ty, seq, &mut reply) {
        Ok(n) => {
            c.color(CYAN);
            c.print(&reply[..4]);
            c.color(GREY);
            // payload starts after the 12-byte header; trim zero padding
            let mut end = n;
            while end > 12 && reply[end - 1] == 0 {
                end -= 1;
            }
            if end > 12 {
                c.print(b"  ");
                c.print(&reply[12..end]);
            }
            c.print(b"\n");
        }
        Err(e) => {
            c.color(RED);
            c.print(b"  envelope bus error: ");
            c.print(e);
            c.print(b"\n");
            c.color(GREY);
        }
    }
}

/// Run the console. Never returns.
pub(crate) unsafe fn run(st: *mut EfiSystemTable, facts: &BootFacts) -> ! {
    let c = Con {
        out: (*st).con_out as *mut TextOut,
        inp: (*st).con_in as *mut SimpleTextInput,
        bs: (*st).boot_services as *mut BootSvc,
        rt: (*st).runtime_services as *mut RuntimeSvc,
    };
    // 1. The watchdog: without this the firmware resets the machine 5 minutes after launch.
    //    The firmware's status is reported as returned, not assumed.
    let wd = if c.bs.is_null() {
        Watchdog::NoBootServices
    } else {
        match ((*c.bs).set_watchdog_timer)(0, 0, 0, core::ptr::null()) {
            0 => Watchdog::Disabled,
            EFI_UNSUPPORTED => Watchdog::Unsupported,
            _ => Watchdog::Failed,
        }
    };
    crate::serial_print(b"  CONSOLE|watchdog=");
    crate::serial_print(watchdog_text(wd));
    crate::serial_print(b"|input=uefi_conin|e=0|json=0\r\n");
    // 2. Splash stays up 1.5 s, then the console takes the screen.
    c.stall_us(1_500_000);
    if !c.inp.is_null() {
        ((*c.inp).reset)(c.inp, false);
    }
    banner(&c, facts, wd);

    let mut seq: u64 = 1;
    // Start from an empty bus.
    init::drain();
    let mut line = [0u8; 64];
    let mut len = 0usize;
    c.color(AMBER);
    c.print(b"asolaria> ");
    c.color(GREY);
    loop {
        let Some(k) = c.key() else {
            c.stall_us(10_000);
            continue;
        };
        let ch = k.unicode_char;
        if ch == 0x0D {
            c.print(b"\n");
            let cmd = &line[..len];
            match cmd {
                b"" => {}
                b"help" | b"?" => help(&c),
                b"ping" => envelope(&c, b"PING", &mut seq),
                b"vers" | b"version" => envelope(&c, b"VERS", &mut seq),
                b"pid" => envelope(&c, b"PIDA", &mut seq),
                b"info" | b"clear" => banner(&c, facts, wd),
                b"reboot" | b"poweroff" => {
                    let _ =
                        asolaria_kernel_core::syscall::sys_cosign_append(b"console-reset-request");
                    c.print(b"  resetting via UEFI ResetSystem\n");
                    crate::serial_print(b"  CONSOLE|reset=requested|e=0|json=0\r\n");
                    c.stall_us(500_000);
                    if !c.rt.is_null() {
                        let kind = if cmd == b"poweroff" { 2 } else { 0 };
                        let _ = ((*c.rt).reset_system)(kind, 0, 0, core::ptr::null());
                    }
                    c.color(RED);
                    c.print(b"  reset did not happen\n");
                    c.color(GREY);
                }
                _ => {
                    c.color(RED);
                    c.print(b"  unknown command: ");
                    c.print(cmd);
                    c.print(b"   (type 'help')\n");
                    c.color(GREY);
                }
            }
            len = 0;
            c.color(AMBER);
            c.print(b"asolaria> ");
            c.color(GREY);
        } else if ch == 0x08 {
            if len > 0 {
                len -= 1;
                c.print(b"\x08 \x08");
            }
        } else if (0x20..0x7F).contains(&ch) && len < line.len() {
            let b = ch as u8;
            line[len] = b;
            len += 1;
            c.print(&[b]);
        }
    }
}
