//! BootProjection = device + time. `device_pid` (hwinv — the 60D-coordinate intern) is STABLE
//! per machine; `boot_pid` is UNIQUE per boot: `boot_pid = sha256(device_pid_full || utc || tsc)[..8]`.
//! Same device, different boot → different boot_pid, because the clock differs. Emits a BOOTPROJ
//! row. E=0: emits, does not register into the live fabric.

use crate::hwinv::{put_bytes, put_hex8, sha256};
use crate::rtc::BootTime;
use crate::serial_print;

/// Mint the device+time boot projection and emit BOOTPROJ. Returns the 8-byte boot_pid.
pub(crate) fn emit(device_digest: &[u8; 32], bt: &BootTime) -> [u8; 8] {
    // preimage = device_pid_full(32) || utc(ascii, 20) || tsc(8 big-endian)
    let mut pre = [0u8; 72];
    let mut n = 32;
    pre[..32].copy_from_slice(device_digest);
    n += bt.write_utc(&mut pre[n..]);
    pre[n..n + 8].copy_from_slice(&bt.tsc.to_be_bytes());
    n += 8;
    let digest = sha256(&pre[..n]);
    let mut boot_pid = [0u8; 8];
    boot_pid.copy_from_slice(&digest[..8]);

    let mut row = [0u8; 160];
    let mut i = put_bytes(&mut row, 0, b"  BOOTPROJ|boot_pid=");
    for &b in &boot_pid {
        i = put_hex8(&mut row, i, b);
    }
    i = put_bytes(&mut row, i, b"|device_pid=");
    for &b in &device_digest[..8] {
        i = put_hex8(&mut row, i, b);
    }
    i = put_bytes(&mut row, i, b"|utc=");
    i += bt.write_utc(&mut row[i..]);
    i = put_bytes(
        &mut row,
        i,
        b"|volatile=1|stable_ref=device_pid|e=0|fire=0|json=0\r\n",
    );
    // SAFETY: serial_print writes COM1 via port I/O; no memory or device-state hazard.
    unsafe {
        serial_print(&row[..i]);
    }
    boot_pid
}
