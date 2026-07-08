//! Early-boot PCI hardware inventory + device_pid mint · pre-driver metal-readiness
//! (acer first-light).
//!
//! Enumerates the PCI configuration space via the legacy 0xCF8/0xCFC port-I/O
//! mechanism, prints every present device (vendor:device + class:subclass) over
//! COM1 serial, and mints a **hardware-specific, PID-specific `device_pid`** from the
//! enumeration: `device_pid = sha256(canonical hardware tuple)[..8]` (the 8-byte Host8
//! handle) with the full sha256 as the proof id. On acer (Nitro AN515-52) metal the
//! tuple includes the PCI-visible Intel RST/VMD storage controller (8086:282A); under
//! QEMU it is the emulated q35 set (proves the code path). Same hardware -> same
//! `device_pid`; different machine -> different pid (D15 DEVICE / D16 PID / D21 HARDWARE).
//!
//! NOTE (liris bilateral review): legacy config enumeration sees the RST *bridge* only —
//! an NVMe REMAPPED behind VMD stays hidden until a VMD driver walks the VMD domain, so
//! this pass does NOT prove NVMe visibility.
//!
//! READ-ONLY re DEVICE STATE + E=0: reads PCI config *data* only (the 0xCF8 address-select
//! write is part of the read protocol; NO config-data / BAR / MMIO / storage writes). The
//! `device_pid` mint is a pure sha256 over the read-only enumeration and is emitted as a
//! `BOOTPID` row with `e=0|fire=0` — it does NOT register into the live fabric and claims
//! no council seal (fabric council_query 2026-07-08 returned sig_verdict=UNSIGNED). DESIGN.

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

pub(crate) fn put_hex8(buf: &mut [u8], i: usize, v: u8) -> usize {
    buf[i] = HEX[(v >> 4) as usize];
    buf[i + 1] = HEX[(v & 0x0F) as usize];
    i + 2
}

fn put_hex16(buf: &mut [u8], i: usize, v: u16) -> usize {
    let i = put_hex8(buf, i, (v >> 8) as u8);
    put_hex8(buf, i, (v & 0xFF) as u8)
}

pub(crate) fn put_bytes(buf: &mut [u8], i: usize, s: &[u8]) -> usize {
    let mut i = i;
    for &c in s {
        buf[i] = c;
        i += 1;
    }
    i
}

fn put_u32(buf: &mut [u8], i: usize, v: u32) -> usize {
    if v == 0 {
        buf[i] = b'0';
        return i + 1;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0;
    let mut x = v;
    while x > 0 {
        tmp[n] = b'0' + (x % 10) as u8;
        x /= 10;
        n += 1;
    }
    let mut i = i;
    while n > 0 {
        n -= 1;
        buf[i] = tmp[n];
        i += 1;
    }
    i
}

// ---------------------------------------------------------------- hardware tuple
/// Cap on the canonical tuple: 6 bytes per device-function, so 512 covers 85 functions
/// (a laptop presents far fewer). The sha256 buffer below is sized for this cap.
const TUPLE_CAP: usize = 512;

/// The canonical hardware tuple accumulated during enumeration, in bus/dev/func order:
/// 6 bytes per present device-function `[vendor_hi, vendor_lo, device_hi, device_lo,
/// class, subclass]`. Deterministic for a given machine -> stable `device_pid`.
struct HwTuple {
    buf: [u8; TUPLE_CAP],
    len: usize,
    count: u32,
    rst_vmd: bool, // Intel RST/VMD controller 8086:282A present (acer storage gap)
    storage: bool, // any PCI class 0x01 (mass storage) present
}

impl HwTuple {
    const fn new() -> Self {
        Self {
            buf: [0u8; TUPLE_CAP],
            len: 0,
            count: 0,
            rst_vmd: false,
            storage: false,
        }
    }
    fn push(&mut self, vendor: u16, device: u16, class: u8, subclass: u8) {
        let b = [
            (vendor >> 8) as u8,
            (vendor & 0xFF) as u8,
            (device >> 8) as u8,
            (device & 0xFF) as u8,
            class,
            subclass,
        ];
        if self.len + b.len() <= TUPLE_CAP {
            self.buf[self.len..self.len + b.len()].copy_from_slice(&b);
            self.len += b.len();
        }
        self.count += 1;
        if vendor == 0x8086 && device == 0x282A {
            self.rst_vmd = true;
        }
        if class == 0x01 {
            self.storage = true;
        }
    }
}

// ---------------------------------------------------------------- sha256 (no_std, no-alloc)
/// Minimal fixed-buffer sha256. Input is bounded by `TUPLE_CAP`; the padded message fits
/// in a 640-byte stack buffer, so no heap is used (the boot heap is a 16 KiB bump arena).
pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = [0u8; 640];
    let n = if data.len() > TUPLE_CAP {
        TUPLE_CAP
    } else {
        data.len()
    };
    msg[..n].copy_from_slice(&data[..n]);
    msg[n] = 0x80;
    let mut total = n + 1;
    while total % 64 != 56 {
        total += 1;
    }
    let bit_len = (n as u64).wrapping_mul(8);
    msg[total..total + 8].copy_from_slice(&bit_len.to_be_bytes());
    total += 8;

    let mut off = 0;
    while off < total {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().enumerate().take(16) {
            let j = off + i * 4;
            *word = u32::from_be_bytes([msg[j], msg[j + 1], msg[j + 2], msg[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
        off += 64;
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Project the hardware tuple into a 60D Brown-Hilbert coordinate: 60 × 10-bit glyphs,
/// counter-salted sha256 — the same 60-tuple frame the kernel's BEHCS-1024 PIDs use
/// (`envelope`/`spawn_gate`/`syscall`/`hookwall`). Makes `dims=60` a real coordinate. No alloc.
fn project_60d(tuple: &[u8]) -> [u16; 60] {
    let mut sel = [0u16; 60];
    let mut buf = [0u8; TUPLE_CAP + 4];
    let n = if tuple.len() > TUPLE_CAP {
        TUPLE_CAP
    } else {
        tuple.len()
    };
    buf[..n].copy_from_slice(&tuple[..n]);
    let (mut i, mut counter) = (0usize, 0u32);
    while i < 60 {
        buf[n..n + 4].copy_from_slice(&counter.to_be_bytes());
        let h = sha256(&buf[..n + 4]);
        let mut j = 0;
        while j + 1 < 32 && i < 60 {
            sel[i] = (((h[j] as u16) << 8) | h[j + 1] as u16) & 0x03FF;
            i += 1;
            j += 2;
        }
        counter += 1;
    }
    sel
}

/// Mint the `device_pid` from the accumulated tuple and emit the `BOOTPID` hot-path row.
/// `device_pid` = sha256(tuple)[..8] as hex16 (the 8-byte Host8 handle); the full sha256
/// is the proof id. E=0: emitted, not fired; no live-fabric registration, no seal.
fn emit_device_pid(t: &HwTuple) -> [u8; 32] {
    // Raw hardware-tuple sha256 — kept as the PROOF of the underlying hardware.
    let hw_digest = sha256(&t.buf[..t.len]);
    // 60D Brown-Hilbert coordinate of the hardware tuple. device_pid IS the intern of THIS
    // coordinate (sha256(coord)[..8]) — identity derived FROM the 60D frame, not a raw-tuple sha
    // with a 60D field bolted beside it (liris bilateral review 2026-07-08).
    let coord = project_60d(&t.buf[..t.len]);
    let mut coord_bytes = [0u8; 120];
    for (k, g) in coord.iter().enumerate() {
        coord_bytes[k * 2] = (g >> 8) as u8;
        coord_bytes[k * 2 + 1] = (g & 0xFF) as u8;
    }
    let device_digest = sha256(&coord_bytes);
    let mut row = [0u8; 448];
    let mut i = put_bytes(&mut row, 0, b"  BOOTPID|device_pid=");
    for &byte in &device_digest[..8] {
        i = put_hex8(&mut row, i, byte); // device_pid = intern of the 60D coordinate
    }
    i = put_bytes(&mut row, i, b"|device_pid_full=");
    for &byte in &device_digest {
        i = put_hex8(&mut row, i, byte); // full sha256 of the 60D coordinate
    }
    i = put_bytes(&mut row, i, b"|hw_tuple_pid_full=");
    for &byte in &hw_digest {
        i = put_hex8(&mut row, i, byte); // raw hardware-tuple sha256 = the proof
    }
    i = put_bytes(&mut row, i, b"|hw_devices=");
    i = put_u32(&mut row, i, t.count);
    i = put_bytes(&mut row, i, b"|rst_vmd=");
    row[i] = if t.rst_vmd { b'1' } else { b'0' };
    i += 1;
    i = put_bytes(&mut row, i, b"|storage=");
    row[i] = if t.storage { b'1' } else { b'0' };
    i += 1;
    i = put_bytes(
        &mut row,
        i,
        b"|colony=acer|behcs=1024|dims=60|derived_from=60d_coordinate|coord_prefix=",
    );
    for (k, g) in coord.iter().take(6).enumerate() {
        if k > 0 {
            row[i] = b'.';
            i += 1;
        }
        i = put_u32(&mut row, i, *g as u32);
    }
    i = put_bytes(&mut row, i, b"|e=0|fire=0|json=0\r\n");
    // SAFETY: serial_print writes COM1 via port I/O; no memory or device-state hazard.
    unsafe {
        serial_print(&row[..i]);
    }
    device_digest
}

/// Summary handed to the boot orchestrator: the device_pid digest + the RST/VMD gate, so
/// the time / projection / driver stages key off the SAME enumeration.
pub(crate) struct HwSummary {
    pub device_digest: [u8; 32],
    pub rst_vmd: bool,
}

/// Enumerate PCI config space, print each present device over serial, and mint the
/// `device_pid`. Read-only re device state; E=0. Returns the [`HwSummary`].
///
/// Caps at 64 device-functions so a pathological bus map can't flood serial; acer
/// presents well under that. The `device_pid` is minted from whatever was enumerated.
pub unsafe fn pci_scan() -> HwSummary {
    serial_print(b"  hwinv . PCI enumeration (0xCF8/0xCFC, read-only)\r\n");
    let mut tuple = HwTuple::new();
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

                tuple.push(vendor, device, class, subclass);
                if tuple.count >= 64 {
                    serial_print(b"  hwinv . (capped at 64 device-functions)\r\n");
                    let device_digest = emit_device_pid(&tuple);
                    return HwSummary {
                        device_digest,
                        rst_vmd: tuple.rst_vmd,
                    };
                }
            }
        }
    }
    serial_print(b"  hwinv . PCI scan complete\r\n");
    let device_digest = emit_device_pid(&tuple);
    HwSummary {
        device_digest,
        rst_vmd: tuple.rst_vmd,
    }
}
