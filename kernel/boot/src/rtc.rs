//! "Time for our OS" — the real metal clock: wall-clock from the CMOS RTC (ports
//! 0x70/0x71) + a monotonic tick from the CPU Time Stamp Counter (`rdtsc`).
//!
//! A boot is a device AT A TIME. `device_pid` (hwinv) is STABLE across boots;
//! `boot_pid` (bootproj) is device+time and UNIQUE per boot. This module reads the
//! actual hardware clock — not a host `date` — so every BOOT* row is time-specific from
//! the metal itself. Read-only: CMOS index/data reads and `rdtsc`; no writes.

use crate::hwinv::put_bytes;
use crate::serial_print;

/// Wall-clock (UTC as the RTC holds it) + a monotonic TSC tick captured at boot.
pub(crate) struct BootTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    pub tsc: u64,
}

/// 8-bit port write (`out dx, al`).
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

/// 8-bit port read (`in al, dx`).
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

/// Read one CMOS register. NMI-safe: bit 7 of the index port stays clear.
unsafe fn cmos_read(reg: u8) -> u8 {
    outb(0x70, reg & 0x7F);
    inb(0x71)
}

fn bcd_to_bin(v: u8) -> u8 {
    (v & 0x0F) + ((v >> 4) * 10)
}

/// Read the 64-bit Time Stamp Counter (monotonic since power-on).
fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: rdtsc has no memory effects and is available on every x86_64.
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    ((hi as u64) << 32) | (lo as u64)
}

impl BootTime {
    /// Write canonical `YYYY-MM-DDTHH:MM:SSZ` (20 bytes) into `buf`; returns the length.
    pub(crate) fn write_utc(&self, buf: &mut [u8]) -> usize {
        let mut i = put_u4(buf, 0, self.year);
        buf[i] = b'-';
        i += 1;
        i = put_2(buf, i, self.month);
        buf[i] = b'-';
        i += 1;
        i = put_2(buf, i, self.day);
        buf[i] = b'T';
        i += 1;
        i = put_2(buf, i, self.hour);
        buf[i] = b':';
        i += 1;
        i = put_2(buf, i, self.min);
        buf[i] = b':';
        i += 1;
        i = put_2(buf, i, self.sec);
        buf[i] = b'Z';
        i += 1;
        i
    }
}

/// Read the boot wall-clock from the CMOS RTC + a TSC tick, and emit a BOOTTIME row.
pub(crate) unsafe fn read_boot_time() -> BootTime {
    // Wait out any update-in-progress (status A bit 7) so the fields are coherent.
    let mut guard = 0u32;
    while cmos_read(0x0A) & 0x80 != 0 && guard < 1_000_000 {
        guard += 1;
    }
    let raw_sec = cmos_read(0x00);
    let raw_min = cmos_read(0x02);
    let raw_hour = cmos_read(0x04);
    let raw_day = cmos_read(0x07);
    let raw_month = cmos_read(0x08);
    let raw_year = cmos_read(0x09);
    let raw_century = cmos_read(0x32); // 0 when the RTC exposes no century register
    let status_b = cmos_read(0x0B);

    let is_bcd = status_b & 0x04 == 0; // bit 2 clear -> fields are BCD
    let is_12h = status_b & 0x02 == 0; // bit 1 clear -> 12-hour mode
    let conv = |v: u8| if is_bcd { bcd_to_bin(v) } else { v };

    let sec = conv(raw_sec);
    let min = conv(raw_min);
    // In 12h mode the PM flag is bit 7 of the RAW hour byte; strip it before converting.
    let pm = is_12h && (raw_hour & 0x80 != 0);
    let mut hour = conv(raw_hour & 0x7F);
    if is_12h {
        if hour == 12 {
            hour = 0; // 12 AM -> 00
        }
        if pm {
            hour += 12; // PM -> +12 (12 PM stays 12)
        }
    }
    let day = conv(raw_day);
    let month = conv(raw_month);
    let yy = conv(raw_year) as u16;
    let century = if raw_century != 0 {
        conv(raw_century) as u16
    } else {
        20 // assume 21st century when the RTC has no century register
    };
    let year = century * 100 + yy;

    let bt = BootTime {
        year,
        month,
        day,
        hour,
        min,
        sec,
        tsc: rdtsc(),
    };
    emit_boottime(&bt);
    bt
}

/// Emit `BOOTTIME|utc=YYYY-MM-DDTHH:MM:SSZ|tsc=<dec>|src=cmos_rtc+tsc|json=0`.
fn emit_boottime(bt: &BootTime) {
    let mut row = [0u8; 96];
    let mut i = put_bytes(&mut row, 0, b"  BOOTTIME|utc=");
    i += bt.write_utc(&mut row[i..]);
    i = put_bytes(&mut row, i, b"|tsc=");
    i = put_u64(&mut row, i, bt.tsc);
    i = put_bytes(&mut row, i, b"|src=cmos_rtc+tsc|e=0|json=0\r\n");
    // SAFETY: serial_print writes COM1 via port I/O; no memory or device-state hazard.
    unsafe {
        serial_print(&row[..i]);
    }
}

fn put_2(buf: &mut [u8], i: usize, v: u8) -> usize {
    buf[i] = b'0' + (v / 10) % 10;
    buf[i + 1] = b'0' + v % 10;
    i + 2
}

fn put_u4(buf: &mut [u8], i: usize, v: u16) -> usize {
    buf[i] = b'0' + ((v / 1000) % 10) as u8;
    buf[i + 1] = b'0' + ((v / 100) % 10) as u8;
    buf[i + 2] = b'0' + ((v / 10) % 10) as u8;
    buf[i + 3] = b'0' + (v % 10) as u8;
    i + 4
}

fn put_u64(buf: &mut [u8], i: usize, v: u64) -> usize {
    if v == 0 {
        buf[i] = b'0';
        return i + 1;
    }
    let mut tmp = [0u8; 20];
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
