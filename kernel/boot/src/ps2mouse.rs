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

/// Send one byte to the mouse and return its reply (0xFA = ACK).
unsafe fn aux_write(v: u8) -> Option<u8> {
    if !controller_cmd(0xD4) || !wait_input_clear() {
        return None;
    }
    outb(DATA, v);
    wait_aux_byte()
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
    pub(crate) unsafe fn probe() -> (Option<Ps2>, Probe) {
        if inb(STAT) == 0xFF {
            return (None, Probe::NoController);
        }
        // Drop any stale aux bytes left from firmware.
        for _ in 0..16 {
            if inb(STAT) & AUX_FULL != AUX_FULL {
                break;
            }
            let _ = inb(DATA);
        }
        if !controller_cmd(0xA8) {
            return (None, Probe::NoController);
        }
        if aux_write(0xF6) != Some(0xFA) || aux_write(0xF4) != Some(0xFA) {
            return (None, Probe::NoAck);
        }
        (
            Some(Ps2 {
                pkt: [0; 3],
                n: 0,
                left: false,
                right: false,
            }),
            Probe::Ready,
        )
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
