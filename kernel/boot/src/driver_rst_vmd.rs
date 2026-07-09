//! Intel RST/VMD storage driver — device-gated + time-stamped SCAFFOLD (acer-specific).
//!
//! DEVICE-specific: engages only when the device's tuple shows the RST/VMD controller
//! `8086:282A` (`hwinv` `rst_vmd=1`); on any other machine it reports `NOT_APPLICABLE`.
//! TIME-specific: every driver event is stamped with this boot's `boot_pid` + UTC, so a driver
//! load is a (device, time) event.
//!
//! SCAFFOLD ONLY (E=0, `writes=0`): emits the driver's gate + intent. The real work — walking the
//! VMD domain config space to reveal the NVMe REMAPPED behind the bridge, then bringing up the
//! NVMe queues — is the tracked next step and requires physical acer metal to verify.

use crate::hwinv::{put_bytes, put_hex8, HwSummary};
use crate::rtc::BootTime;
use crate::serial_print;

/// Emit the device-gated + time-stamped BOOTDRIVER scaffold row.
pub(crate) fn probe(hw: &HwSummary, boot_pid: &[u8; 8], bt: &BootTime) {
    let mut row = [0u8; 256];
    let mut i = put_bytes(
        &mut row,
        0,
        b"  BOOTDRIVER|driver=intel-rst-vmd|match=8086:282A|device_pid=",
    );
    for &b in &hw.device_digest[..8] {
        i = put_hex8(&mut row, i, b);
    }
    i = put_bytes(&mut row, i, b"|boot_pid=");
    for &b in boot_pid {
        i = put_hex8(&mut row, i, b);
    }
    i = put_bytes(&mut row, i, b"|utc=");
    i += bt.write_utc(&mut row[i..]);
    i = put_bytes(&mut row, i, b"|status=");
    if hw.rst_vmd {
        i = put_bytes(&mut row, i, b"GATED_MATCH|vmd_walk=TODO_next_step");
    } else {
        i = put_bytes(&mut row, i, b"NOT_APPLICABLE|no_8086:282A_on_this_device");
    }
    i = put_bytes(&mut row, i, b"|writes=0|e=0|fire=0|json=0\r\n");
    // SAFETY: serial_print writes COM1 via port I/O; no memory or device-state hazard.
    unsafe {
        serial_print(&row[..i]);
    }
}
