//! Early-boot PCI hardware inventory · pre-driver metal-readiness (acer first-light).
//!
//! Enumerates the PCI configuration space via the legacy 0xCF8/0xCFC port-I/O
//! mechanism and prints every present device (vendor:device + class:subclass)
//! over COM1 serial. This is the "dump the hardware inventory before we trust any
//! driver" step: on acer (Nitro AN515-52) metal it reveals the Intel RST/VMD
//! storage controller (8086:282A) and the NVMe behind it; under QEMU it lists the
//! emulated q35 devices, which proves the code path.
//!
//! READ-ONLY BY CONSTRUCTION: this module issues PCI *config reads* only. It never
//! writes config space, never touches a BAR / MMIO region, and never issues a
//! storage command. It cannot mutate any disk. It returns to the caller so normal
//! init proceeds unchanged.

use crate::serial_print;

const PCI_CFG_ADDR: u16 = 0xCF8;
const PCI_CFG_DATA: u16 = 0xCFC;

/// 32-bit port write (`out dx, eax`). Boot crate only; `core` forbids unsafe.
unsafe fn outl(port: u16, val: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack, preserves_flags));
}

/// 32-bit port read (`in eax, dx`).
unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    core::arch::asm!("in eax, dx", out("eax") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

/// Read one 32-bit dword from PCI config space. `off` is dword-aligned (low 2 bits ignored).
unsafe fn cfg_read32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let addr: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    outl(PCI_CFG_ADDR, addr);
    inl(PCI_CFG_DATA)
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn put_hex8(buf: &mut [u8], i: usize, v: u8) -> usize {
    buf[i] = HEX[(v >> 4) as usize];
    buf[i + 1] = HEX[(v & 0x0F) as usize];
    i + 2
}

fn put_hex16(buf: &mut [u8], i: usize, v: u16) -> usize {
    let i = put_hex8(buf, i, (v >> 8) as u8);
    put_hex8(buf, i, (v & 0xFF) as u8)
}

fn put_bytes(buf: &mut [u8], i: usize, s: &[u8]) -> usize {
    let mut i = i;
    for &c in s {
        buf[i] = c;
        i += 1;
    }
    i
}

/// Enumerate PCI config space and print each present device over serial. Read-only.
///
/// Caps output at 64 device-functions so a pathological bus map can't flood the
/// serial line; acer presents well under that.
pub unsafe fn pci_scan() {
    serial_print(b"  hwinv . PCI enumeration (0xCF8/0xCFC, read-only)\r\n");
    let mut count: u32 = 0;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            let id0 = cfg_read32(bus, dev, 0, 0x00);
            if (id0 & 0xFFFF) == 0xFFFF {
                continue; // no device at function 0 -> skip slot
            }
            // header type bit 7 = multi-function device
            let header_type = ((cfg_read32(bus, dev, 0, 0x0C) >> 16) & 0xFF) as u8;
            let max_func = if header_type & 0x80 != 0 { 8 } else { 1 };
            for func in 0u8..max_func {
                let id = cfg_read32(bus, dev, func, 0x00);
                let vendor = (id & 0xFFFF) as u16;
                if vendor == 0xFFFF {
                    continue;
                }
                let device = (id >> 16) as u16;
                let class_reg = cfg_read32(bus, dev, func, 0x08);
                let class = ((class_reg >> 24) & 0xFF) as u8;
                let subclass = ((class_reg >> 16) & 0xFF) as u8;

                // "  PCI bb:dd.f  vvvv:dddd class=cc:ss\r\n"
                let mut line = [0u8; 48];
                let mut i = put_bytes(&mut line, 0, b"  PCI ");
                i = put_hex8(&mut line, i, bus);
                line[i] = b':';
                i += 1;
                i = put_hex8(&mut line, i, dev);
                line[i] = b'.';
                i += 1;
                line[i] = b'0' + func;
                i += 1;
                i = put_bytes(&mut line, i, b"  ");
                i = put_hex16(&mut line, i, vendor);
                line[i] = b':';
                i += 1;
                i = put_hex16(&mut line, i, device);
                i = put_bytes(&mut line, i, b" class=");
                i = put_hex8(&mut line, i, class);
                line[i] = b':';
                i += 1;
                i = put_hex8(&mut line, i, subclass);
                i = put_bytes(&mut line, i, b"\r\n");
                serial_print(&line[..i]);

                count += 1;
                if count >= 64 {
                    serial_print(b"  hwinv . (capped at 64 device-functions)\r\n");
                    return;
                }
            }
        }
    }
    serial_print(b"  hwinv . PCI scan complete\r\n");
}
