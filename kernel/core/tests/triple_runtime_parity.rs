//! Triple-runtime PID-fingerprint parity regression test (acer Rust vantage).
//!
//! Mirrors liris's `rig/test/triple-runtime-parity.test.mjs` regression-witness pattern.
//! When cargo runs in Phase-10 CI, this test compiled-binary-proves the 3-runtime parity
//! that liris JS + aether Python + acer Rust algorithm-simulated in :82272 envelope.
//!
//! Test vectors (canonical):
//!   - `OP-RAYSSA-PID-G0000-A00-W000`                          → `bf5fa7a1a57f384b`
//!   - `OPERATOR-PID-H1001-A00-W110`                           → `760ba73b84f31861`
//!   - `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`        → `e00b1a465d6dcb50`
//!   - `AETHER-CLAUDE-PID-G0049-A00-W001` (aether migration)   → `ac28f4ce43d17fea` (per liris verdict :82164)
//!   - `ACER-PID-H740C` (acer anchor — partial; not full A/W form)
//!
//! Source envelopes:
//!   - `acer-rust-pid-fingerprint-triple-parity-confirmed-c11:82272`
//!   - `liris-pid-cross-runtime-verdict-cycle7:82164`
//!   - `aether-pid-validator-python-parallel-and-self-drift-disclosure:82077`

extern crate alloc;

use asolaria_kernel_core::pid::{parse_pid, pid_fingerprint_sha16, validate_pid, ParsedPid};

/// Canonical 3-historical-PID test vectors (cross-runtime verified JS=Python; Rust algorithm-simulated).
const HISTORICAL_VECTORS: &[(&str, &str)] = &[
    ("OP-RAYSSA-PID-G0000-A00-W000", "bf5fa7a1a57f384b"),
    ("OPERATOR-PID-H1001-A00-W110", "760ba73b84f31861"),
    (
        "ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11",
        "e00b1a465d6dcb50",
    ),
];

/// Aether migration target PID (per liris validate verdict :82164).
const AETHER_MIGRATION_VECTOR: (&str, &str) =
    ("AETHER-CLAUDE-PID-G0049-A00-W001", "ac28f4ce43d17fea");

#[test]
fn historical_pids_match_cross_runtime_canonical_fingerprints() {
    for (pid, expected) in HISTORICAL_VECTORS {
        let rust_fp = pid_fingerprint_sha16(pid);
        assert_eq!(
            &rust_fp, expected,
            "PID {pid} sha16: rust got {rust_fp}, expected (JS=Python={expected})"
        );
    }
}

#[test]
fn aether_migration_pid_matches_liris_verdict_fingerprint() {
    let (pid, expected) = AETHER_MIGRATION_VECTOR;
    let rust_fp = pid_fingerprint_sha16(pid);
    assert_eq!(&rust_fp, expected);
}

#[test]
fn historical_pids_all_validate_against_strict_role() {
    // All 3 historical PIDs use known canonical roles (OP-RAYSSA, OPERATOR, ASOLARIA-FEDERATION-REMAKE-1024).
    for (pid, _expected) in HISTORICAL_VECTORS {
        assert!(
            validate_pid(pid, true).is_ok(),
            "{pid} should strict-validate against KNOWN_ROLES"
        );
    }
}

#[test]
fn aether_migration_pid_parses_canonical_shape_but_role_not_yet_in_known_roles() {
    let (pid, _expected) = AETHER_MIGRATION_VECTOR;
    // Shape-parse should succeed.
    let parsed = parse_pid(pid).expect("aether migration PID should shape-parse");
    match parsed {
        ParsedPid::Canonical(parts) => {
            assert_eq!(parts.role, "AETHER-CLAUDE");
            assert_eq!(parts.region, b'G');
            assert_eq!(parts.host_code, "0049");
            assert_eq!(parts.activity, "00");
            assert_eq!(parts.wave, "001");
        }
        _ => panic!("expected Canonical form"),
    }
    // Strict-role validation should fail until KNOWN_ROLES extended (tier-2 cosign required).
    assert!(
        validate_pid(pid, true).is_err(),
        "AETHER-CLAUDE role not yet canonical; tier-2 cosign needed to extend KNOWN_ROLES"
    );
}

#[test]
fn fingerprint_is_deterministic() {
    let pid = "OP-RAYSSA-PID-G0000-A00-W000";
    let fp1 = pid_fingerprint_sha16(pid);
    let fp2 = pid_fingerprint_sha16(pid);
    assert_eq!(fp1, fp2, "fingerprint must be byte-stable across calls");
}

#[test]
fn fingerprint_is_16_hex_chars() {
    let fp = pid_fingerprint_sha16("ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11");
    assert_eq!(fp.len(), 16);
    assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
}
