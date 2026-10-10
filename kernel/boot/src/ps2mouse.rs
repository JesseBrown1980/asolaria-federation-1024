//! PS/2 auxiliary-port mouse driver (i8042, ports 0x60/0x64) — the kernel's own pointer driver,
//! used only when the firmware exposes no physical pointer device.
//!
//! Why it exists: stock OVMF has no USB-mouse or PS/2-mouse UEFI driver (measured: only the
//! console splitter's empty virtual pointer), and many laptop touchpads are wired to the
//! keyboard controller's auxiliary port in their "basic"/PS/2 mode. This driver enables the aux
//! port, turns on stream mode, and decodes standard 3-byte packets.
//!
//! Coexistence with the firmware keyboard: only bytes flagged as aux data (status bit 5) are
//! read; keyboard bytes are left in the controller for the firmware's ConIn driver. Device
//! writes are limited to the standard mouse enable sequence (0xA8, 0xD4 0xF6, 0xD4 0xF4).
//! Integer only. E=0.

use crate::{inb, outb};

const DATA: u16 = 0x60;
const STAT: u16 = 0x64;
/// Status: output buffer full (bit 0) and that byte is from the aux port (bit 5).
const AUX_FULL: u8 = 0x21;

pub(crate) struct Ps2 {
    pkt: [u8; 3],
    n: usize,
    left: bool,
    right: bool,
}

unsafe fn wait_input_clear() -> bool {
    for _ in 0..100_000 {
        if inb(STAT) & 0x02 == 0 {
            return true;
        }
    }
    false
}

unsafe fn wait_aux_byte() -> Option<u8> {
    for _ in 0..200_000 {
        if inb(STAT) & AUX_FULL == AUX_FULL {
            return Some(inb(DATA));
        }
    }
    None
}

unsafe fn controller_cmd(c: u8) -> bool {
    if !wait_input_clear() {
        return false;
    }
    outb(STAT, c);
    true
}

/// Drop pending KEYBOARD bytes (output buffer full, not aux) so an aux ACK can reach the
/// output buffer. Only used around mouse commands (probe / suspend / resume); bounded.
unsafe fn drop_kbd_bytes() {
    for _ in 0..16 {
        if inb(STAT) & AUX_FULL != 0x01 {
            break;
        }
        let _ = inb(DATA);
    }
}

/// Bounded drain of pending aux (mouse) bytes; returns how many were dropped. The text
/// console calls this every idle tick so a stray mouse byte can never sit in the controller's
/// single output buffer and starve the firmware keyboard driver.
pub(crate) unsafe fn drain_aux() -> usize {
    let mut n = 0;
    while n < 32 && inb(STAT) & AUX_FULL == AUX_FULL {
        let _ = inb(DATA);
        n += 1;
    }
    n
}

/// Send one byte to the mouse and wait for its ACK (0xFA). Stray movement bytes that were
/// already queued ahead of the ACK are skipped (bounded).
unsafe fn aux_cmd(v: u8) -> bool {
    drop_kbd_bytes();
    if !controller_cmd(0xD4) || !wait_input_clear() {
        return false;
    }
    outb(DATA, v);
    for _ in 0..8 {
        match wait_aux_byte() {
            Some(0xFA) => return true,
            Some(0xFE) | Some(0xFC) | None => return false,
            Some(_) => continue,
        }
    }
    false
}

/// Read the controller configuration byte (command 0x20). None if it never arrived.
unsafe fn read_config() -> Option<u8> {
    drop_kbd_bytes();
    if !controller_cmd(0x20) {
        return None;
    }
    for _ in 0..200_000 {
        let st = inb(STAT);
        if st & 0x01 != 0 {
            return Some(inb(DATA));
        }
    }
    None
}

/// Outcome of the probe, for the on-screen/serial report.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Probe {
    NoController,
    NoAck,
    Ready,
}

impl Ps2 {
    /// Enable the aux port and stream mode. Returns the driver if the mouse acknowledged.
    /// One retry; on failure the aux port is put back the way it was found.
    pub(crate) unsafe fn probe() -> (Option<Ps2>, Probe) {
        if inb(STAT) == 0xFF {
            return (None, Probe::NoController);
        }
        // Config bit 5 set = aux clock disabled before we touched it.
        let was_disabled = read_config().map_or(true, |c| c & 0x20 != 0);
        let _ = drain_aux();
        for _attempt in 0..2 {
            if !controller_cmd(0xA8) {
                return (None, Probe::NoController);
            }
            if aux_cmd(0xF6) && aux_cmd(0xF4) {
                return (
                    Some(Ps2 {
                        pkt: [0; 3],
                        n: 0,
                        left: false,
                        right: false,
                    }),
                    Probe::Ready,
                );
            }
            let _ = drain_aux();
        }
        // Restore: streaming off (best effort), and the aux port disabled again if it was.
        let _ = aux_cmd(0xF5);
        let _ = drain_aux();
        if was_disabled {
            let _ = controller_cmd(0xA7);
        }
        (None, Probe::NoAck)
    }

    /// Stop the mouse before the screen is handed to the firmware text console: streaming off
    /// (0xF5), aux port disabled (0xA7), pending aux bytes dropped. Nothing can then sit in the
    /// controller's output buffer in front of keyboard bytes.
    pub(crate) unsafe fn suspend(&mut self) {
        let _ = aux_cmd(0xF5);
        let _ = controller_cmd(0xA7);
        let _ = drain_aux();
        self.n = 0;
    }

    /// Undo [`suspend`]: aux port on (0xA8), streaming on (0xF4).
    pub(crate) unsafe fn resume(&mut self) {
        let _ = drain_aux();
        let _ = controller_cmd(0xA8);
        let _ = aux_cmd(0xF4);
        self.n = 0;
        self.left = false;
        self.right = false;
    }

    /// Drain waiting aux bytes; returns accumulated (dx, dy, left, right) if a packet arrived.
    /// dy is screen-down positive.
    pub(crate) unsafe fn poll(&mut self) -> Option<(i32, i32, bool, bool)> {
        let (mut dx, mut dy, mut got) = (0i32, 0i32, false);
        for _ in 0..96 {
            if inb(STAT) & AUX_FULL != AUX_FULL {
                break;
            }
            let b = inb(DATA);
            // Packet byte 0 always has bit 3 set: resynchronise on it.
            if self.n == 0 && b & 0x08 == 0 {
                continue;
            }
            self.pkt[self.n] = b;
            self.n += 1;
            if self.n < 3 {
                continue;
            }
            self.n = 0;
            let b0 = self.pkt[0] as i32;
            if b0 & 0xC0 != 0 {
                continue; // overflow: discard the packet
            }
            let x = self.pkt[1] as i32 - ((b0 << 4) & 0x100);
            let y = self.pkt[2] as i32 - ((b0 << 3) & 0x100);
            // Mild integer acceleration so a touchpad crosses the screen in a few strokes.
            let acc = |v: i32| if v.abs() > 6 { v * 2 } else { v };
            dx += acc(x);
            dy -= acc(y);
            let (l, r) = (b0 & 1 != 0, b0 & 2 != 0);
            let changed = (l, r) != (self.left, self.right);
            self.left = l;
            self.right = r;
            got = true;
            // A button change ends this poll, so press/release pairs are never merged into
            // one state (a double-click needs to see both presses).
            if changed {
                break;
            }
        }
        got.then_some((dx, dy, self.left, self.right))
    }
}
