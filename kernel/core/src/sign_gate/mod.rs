//! SIGN-GATE · BEHCS-256 absorb (Rust crypto core port)
//!
//! Source: `C:/asolaria-behcs-256/src/hookwall/sign-gate.js`
//! sha16=`a7c06eca27be3339`, 11693B · FW-05-HOOKWALL-SIGN-GATE-LIVE-PROMOTE-PID-fw-05 (live 2026-05-06).
//!
//! Per Vanguard scout #2: port crypto verify + denies/allows logic to Rust.
//! Keep file I/O + NDJSON event logging in Node.js (those live in `dashboard-improvements-2026-05-11/`).
//!
//! Three-keys context (REPO_LAW Invariant 2 + 4):
//!   hookwall (uniform entry) + PID (uniform addressing) + GNN pipes (uniform flow).
//! Sign-gate is the security check on the uniform entry: ed25519-verify then deny/allow filter.

use crate::crypto::{verify, CryptoErr, PublicKey, Signature};
use alloc::string::String;

/// Default denies — case-sensitive substring tags scanned across `envelope.hookwall_events[].tag`.
pub const DEFAULT_DENIES: &[&str] = &[
    "HD-virus",
    "drift",
    "malformed-PID",
    "unknown-cube",
    "off-fabric",
];

/// Task-#39 narrow allow: envvar names matching the pattern `^[A-Z][A-Z0-9_]*VIRUS[A-Z0-9_]*$`.
/// v0.1 implements substring-bool. v0.2 wires real regex via `regex` crate.
pub const ALLOW_KIND_TASK_39: &str = "envvar-name-virus-suffix";

/// Sign-gate verdict for one envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateVerdict {
    /// Final pass/fail.
    pub admitted: bool,
    /// Brief reason — never empty.
    pub reason: String,
    /// True if signature verified successfully.
    pub sig_valid: bool,
    /// True if no deny-tag matched.
    pub deny_clean: bool,
    /// True if envelope was rescued by an allow rule (overrides a deny).
    pub allow_rescued: bool,
}

/// Sign-gate errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignGateErr {
    /// Envelope canonical bytes empty.
    EnvelopeEmpty,
    /// Algorithm not supported (only ed25519 in v0.1).
    UnsupportedAlg,
    /// Signature missing.
    SigMissing,
    /// Signature owner missing — cannot resolve key.
    OwnerMissing,
}

/// Verify the ed25519 signature over the envelope canonical bytes.
/// Wraps `crypto::verify` with sign-gate-specific error mapping.
pub fn verify_envelope_signature(
    canonical_bytes: &[u8],
    sig: &Signature,
    owner_pub_key: &PublicKey,
    alg: &str,
) -> Result<(), SignGateErr> {
    if canonical_bytes.is_empty() {
        return Err(SignGateErr::EnvelopeEmpty);
    }
    if alg != "ed25519" {
        return Err(SignGateErr::UnsupportedAlg);
    }
    match verify(canonical_bytes, sig, owner_pub_key) {
        Ok(()) => Ok(()),
        Err(CryptoErr::SignatureInvalid) => Err(SignGateErr::SigMissing),
        Err(_) => Err(SignGateErr::SigMissing),
    }
}

/// Check hookwall events against the deny-list. Returns true if ALL events are clean.
pub fn deny_check(hookwall_event_tags: &[&str], denies: &[&str]) -> bool {
    for tag in hookwall_event_tags {
        for deny in denies {
            // Case-sensitive substring match per JS source line 58.
            if tag.contains(deny) {
                return false;
            }
        }
    }
    true
}

/// Task-#39 narrow allow check: envvar name matches `^[A-Z][A-Z0-9_]*VIRUS[A-Z0-9_]*$`.
/// v0.1 simplified rule: name is all-uppercase A-Z/0-9/_ AND starts with letter AND contains "VIRUS".
pub fn task_39_envvar_virus_allow(envvar_name: &str) -> bool {
    let bytes = envvar_name.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    // First char must be A-Z.
    if !(bytes[0] >= b'A' && bytes[0] <= b'Z') {
        return false;
    }
    // All chars must be A-Z, 0-9, or _.
    for &b in bytes {
        let ok = (b >= b'A' && b <= b'Z') || (b >= b'0' && b <= b'9') || b == b'_';
        if !ok {
            return false;
        }
    }
    // Must contain VIRUS as substring.
    envvar_name.contains("VIRUS")
}

/// Combined gate: verify sig + deny-check + allow-rescue.
/// Returns a full `GateVerdict` recording all 3 sub-decisions.
pub fn gate_envelope(
    canonical_bytes: &[u8],
    sig: &Signature,
    owner_pub_key: &PublicKey,
    alg: &str,
    hookwall_event_tags: &[&str],
    envvar_names: &[&str],
) -> GateVerdict {
    let sig_valid = verify_envelope_signature(canonical_bytes, sig, owner_pub_key, alg).is_ok();
    let deny_clean = deny_check(hookwall_event_tags, DEFAULT_DENIES);
    let allow_rescued = !deny_clean && envvar_names.iter().any(|n| task_39_envvar_virus_allow(n));

    let admitted = sig_valid && (deny_clean || allow_rescued);
    let reason = if !sig_valid {
        String::from("signature-invalid")
    } else if !deny_clean && !allow_rescued {
        String::from("deny-tag-matched-no-allow-rescue")
    } else if allow_rescued {
        String::from("deny-tag-matched-but-allow-rescued")
    } else {
        String::from("admitted-clean")
    };

    GateVerdict {
        admitted,
        reason,
        sig_valid,
        deny_clean,
        allow_rescued,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_denies_match_canon() {
        assert_eq!(DEFAULT_DENIES.len(), 5);
        assert!(DEFAULT_DENIES.contains(&"HD-virus"));
        assert!(DEFAULT_DENIES.contains(&"drift"));
        assert!(DEFAULT_DENIES.contains(&"malformed-PID"));
    }

    #[test]
    fn deny_check_clean_tags_pass() {
        assert!(deny_check(&["safe-tag", "all-clear"], DEFAULT_DENIES));
    }

    #[test]
    fn deny_check_substring_match_fails() {
        assert!(!deny_check(&["something-HD-virus-name"], DEFAULT_DENIES));
        assert!(!deny_check(&["drift-detected"], DEFAULT_DENIES));
    }

    #[test]
    fn task_39_uppercase_virus_envvar_allowed() {
        assert!(task_39_envvar_virus_allow("ASOLARIA_VIRUS_SCANNER"));
        assert!(task_39_envvar_virus_allow("VIRUS_DB"));
        assert!(task_39_envvar_virus_allow("MY_VIRUS_FLAG_2"));
    }

    #[test]
    fn task_39_lowercase_or_freetext_rejected() {
        assert!(!task_39_envvar_virus_allow("asolaria_virus_scanner")); // lowercase
        assert!(!task_39_envvar_virus_allow("3VIRUS_NAME")); // starts with digit
        assert!(!task_39_envvar_virus_allow("SOMETHING ELSE")); // contains space
    }

    #[test]
    fn gate_envelope_sig_invalid_rejects() {
        let sig = Signature([0u8; 64]);
        let pk = PublicKey([0u8; 32]);
        // crypto::verify stub returns Unimplemented → SignGateErr::SigMissing → sig_valid=false
        let v = gate_envelope(
            b"canonical-bytes",
            &sig,
            &pk,
            "ed25519",
            &["safe"],
            &["BAR"],
        );
        assert!(!v.sig_valid);
        assert!(!v.admitted);
    }

    #[test]
    fn gate_envelope_empty_bytes_rejects() {
        let sig = Signature([0u8; 64]);
        let pk = PublicKey([0u8; 32]);
        let v = gate_envelope(&[], &sig, &pk, "ed25519", &[], &[]);
        assert!(!v.sig_valid);
        assert!(!v.admitted);
    }
}
