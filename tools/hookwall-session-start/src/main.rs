//! hookwall-session-start — AoT Rust port of the Claude Code `SessionStart` hook
//! (formerly `C:/tmp/hookwall-handlers/session-start.mjs`).
//!
//! Operator mandate (2026-06-23): an 8-byte host process controlling stubbed
//! rooms. No node, no json, no http, no resident service, no fire, no T0.
//!
//! - `SessionStart` injects plain stdout as context (Claude Code hook contract),
//!   so the governing-discipline text is written directly to stdout. The JSON
//!   envelope the Node hook emitted is intentionally dropped (no json).
//! - Internal model is HBP/HBI tuple-text: each rule line and each room is an
//!   8-byte (`u64`) handle. RAM holds only handles + bounded scratch; the rule
//!   corpus is compiled into `.rodata` via `include_str!` (no heap corpus load).
//! - stdin is drained bounded and ignored (the Node hook used it only for the
//!   now-removed best-effort `:4947` curl — no http here).
//! - Rooms (HD / USB-SOVLINUX / Google-Drive / GitHub / Liris-recall) are known
//!   to the hook by 8-byte HBI pointer ONLY. They are never read or loaded during
//!   SessionStart. A separate gated verifier enumerates a room and emits its own
//!   receipt — this hook outputs the pointer law, never room contents.
//!
//! Parity: stdout bytes equal the Node hook's runtime `additionalContext` byte
//! for byte (`rules.txt` is the LF-normalized runtime payload, not retyped).
//! Receipt 2026-06-23: sha256 9184f2320fcf1a693304abba7960886014a9704c8841d77e96b96aeeee8e3cd3.

use std::io::{self, Read, Write};

/// Governing-discipline text, compiled in. Byte-identical to the Node hook's
/// runtime `additionalContext` (the `RULES` template literal, LF-normalized by
/// JS at parse). Extracted from the live hook, never hand-transcribed.
const RULES: &str = include_str!("../rules.txt");

/// Bounded stdin drain ceiling. Content is discarded; only the count is tracked.
const STDIN_CAP: usize = 1 << 16;

/// FNV-1a 64-bit — compile-time 8-byte handle minting for rules and rooms.
const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    hash
}

/// A stubbed room: a byte-corpus lane the host addresses through an 8-byte HBI
/// handle. The hook holds only the handle; it never materializes room bytes.
#[derive(Clone, Copy)]
struct Room {
    kind: &'static str,
    role: &'static str,
    /// Access level: 0 public · 5 federation · 9 owner-private.
    access: u8,
    /// Evidence tag: CANON · MEASURED · OPERATOR_OBSERVED · UNVERIFIED.
    status: &'static str,
    stub: &'static str,
    /// 8-byte handle (HBI pointer) into the room.
    hbi: u64,
}

const fn room(
    kind: &'static str,
    role: &'static str,
    access: u8,
    status: &'static str,
    stub: &'static str,
) -> Room {
    Room {
        kind,
        role,
        access,
        status,
        stub,
        hbi: fnv1a64(stub.as_bytes()),
    }
}

/// The room registry the 8-byte host controls. Stubbed; `load=never_on_session_start`.
const ROOMS: [Room; 5] = [
    room(
        "hd",
        "local_stub_room",
        5,
        "MEASURED",
        "C:/Users/acer/.claude/projects/C--/memory",
    ),
    room(
        "usb",
        "metal_substrate",
        9,
        "CANON",
        "SOVLINUX:/runtime/omni-consolidation-registry",
    ),
    room(
        "google_drive",
        "proof_master_repo",
        9,
        "OPERATOR_OBSERVED",
        "gdrive:/1vhEsLhSTTxeSfc8aICox_wYZSN7aQeX8",
    ),
    room(
        "github",
        "bilateral_merge",
        0,
        "MEASURED",
        "github:/JesseBrown1980/asolaria-federation-1024",
    ),
    room(
        "liris_recall",
        "measured_recall_index",
        5,
        "MEASURED_LOCAL",
        "liris:/HILBRA-IDX-BEHCS-TUPLE-TEXT-V1",
    ),
];

fn main() {
    // Bounded-drain stdin (Claude passes a small SessionStart event). Never parsed
    // or stored — the Node hook used it only for the removed :4947 emit.
    drain_stdin_bounded();

    // Boundary: governing discipline as plain context. No json wrapper.
    let mut out = io::stdout().lock();
    let _ = out.write_all(RULES.as_bytes());
    let _ = out.flush();

    // Room/handle manifest to stderr (debug lane; never pollutes the context
    // payload, never part of parity). HBP tuple-text, json=0. Off by default.
    if std::env::var_os("HOOKWALL_DEBUG").is_some() {
        emit_manifest();
    }
}

fn drain_stdin_bounded() {
    let mut stdin = io::stdin().lock();
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    loop {
        match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total = total.saturating_add(n);
                if total >= STDIN_CAP {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn emit_manifest() {
    let mut err = io::stderr().lock();
    for (i, line) in RULES.lines().enumerate() {
        let handle = fnv1a64(line.as_bytes());
        let len = line.len();
        let _ = writeln!(
            err,
            "HBPv1|kind=rule|idx={i}|handle={handle:016x}|len={len}|json=0"
        );
    }
    for r in &ROOMS {
        let (kind, role, access, status, stub, hbi) =
            (r.kind, r.role, r.access, r.status, r.stub, r.hbi);
        let _ = writeln!(
            err,
            "ROOM|kind={kind}|role={role}|access={access}|status={status}|hbi={hbi:016x}|stub={stub}|load=never_on_session_start|json=0"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_payload_headed_and_no_trailing_newline() {
        assert!(RULES.starts_with("ACER GOVERNING DISCIPLINE"));
        assert!(RULES.contains("CLAIMS GATE"));
        // Byte-parity with the Node `additionalContext`: no trailing newline.
        assert!(!RULES.ends_with('\n'));
    }

    #[test]
    fn handles_are_eight_bytes_distinct_nonzero() {
        assert_eq!(core::mem::size_of::<u64>(), 8);
        let mut hs: Vec<u64> = ROOMS.iter().map(|r| r.hbi).collect();
        let n = hs.len();
        hs.sort_unstable();
        hs.dedup();
        assert_eq!(hs.len(), n, "room handles must be distinct");
        assert!(ROOMS.iter().all(|r| r.hbi != 0));
    }

    #[test]
    fn fnv_known_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn google_drive_room_is_pointer_only_operator_observed() {
        let g = ROOMS
            .iter()
            .find(|r| r.kind == "google_drive")
            .expect("drive room present");
        assert_eq!(g.status, "OPERATOR_OBSERVED");
        assert_eq!(g.access, 9);
        assert!(g.stub.contains("1vhEsLhSTTxeSfc8aICox_wYZSN7aQeX8"));
    }

    /// Hermetic parity: `rules.txt` must equal the runtime `additionalContext`
    /// the Node hook delivers. No node, no network — extract the `RULES` literal
    /// and model JS template-literal line-ending normalization (`\r\n`/`\r` -> `\n`),
    /// which is why the on-disk CRLF source equals the LF runtime payload.
    #[test]
    fn parity_against_node_hook_source_if_present() {
        let path = "C:/tmp/hookwall-handlers/session-start.mjs";
        let Ok(src) = std::fs::read_to_string(path) else {
            return;
        };
        let marker = "const RULES = `";
        let Some(start) = src.find(marker).map(|i| i + marker.len()) else {
            return;
        };
        let Some(rel_end) = src[start..].find('`') else {
            return;
        };
        let normalized = src[start..start + rel_end]
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        assert_eq!(
            normalized, RULES,
            "rules.txt drifted from Node hook runtime RULES"
        );
    }
}
