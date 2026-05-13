//! BEHCS-1024 PID minter · Phase-2 Step 27
//!
//! Rust no_std port of liris's `liris-pid-mint-reference.mjs` (sha16=`fd7c341eabf40e95`, 11/11 tests PASS).
//! Reference path on liris: `C:/Users/rayss/Asolaria/federation-remake-1024/rig/liris-pid-mint-reference.mjs`
//! Source envelope: `LIRIS_COSIGN_PEERS_BATCH_AND_PID_MINTER_UNBLOCK:81906`
//!
//! ## Canonical PID format
//!
//! ```text
//! <ROLE>-PID-<REGION><HOST_CODE>-A<2hex>-W<3hex>
//! ```
//! - `<ROLE>`: uppercase token, hyphens permitted (e.g. `OP-RAYSSA`, `OPERATOR`, `ACER`, `LIRIS`, `FALCON`, `AETHER`)
//! - `<REGION>`: 1 char from `{G, H, F, D}` (PID_NAMESPACE)
//! - `<HOST_CODE>`: 4 hex digits
//! - `-A<2hex>`: activity tag
//! - `-W<3hex>`: wave tag
//!
//! ## Anchor format exception
//!
//! ```text
//! <ROLE>-PID-<YYYY-MM-DD>
//! ```
//! Date-suffixed anchor class (e.g. `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`).
//!
//! ## Historical PIDs validated in liris test vectors (11/11 PASS)
//!
//! - `OP-RAYSSA-PID-G0000-A00-W000`
//! - `OPERATOR-PID-H1001-A00-W110` (first human operator-citizen 2026-04-30)
//! - `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11` (anchor exception)

use alloc::string::String;
use alloc::vec::Vec;

/// PID namespace regions per liris reference.
pub mod namespace {
    pub const GRAND: u8 = b'G';
    pub const HOST: u8 = b'H';
    pub const FEDERATION: u8 = b'F';
    pub const DATA: u8 = b'D';
}

/// Known canonical roles (additions require tier-2 cosign per REPO_LAW Invariant 9).
pub const KNOWN_ROLES: &[&str] = &[
    "OPERATOR",
    "OP-JESSE",
    "OP-RAYSSA",
    "ACER",
    "LIRIS",
    "FALCON",
    "AETHER",
    "ASOLARIA-FEDERATION-REMAKE-1024",
];

/// PID minter errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PidErr {
    /// PID format does not match either canonical or anchor pattern.
    MalformedFormat,
    /// REGION char is not in {G, H, F, D}.
    InvalidRegion,
    /// HOST_CODE is not 4 hex digits.
    InvalidHostCode,
    /// Activity tag is not 2 hex digits.
    InvalidActivity,
    /// Wave tag is not 3 hex digits.
    InvalidWave,
    /// Process tag is not P<2hex>.
    InvalidProcess,
    /// Nonce tag is not N<5hex>.
    InvalidNonce,
    /// Role is not in KNOWN_ROLES.
    UnknownRole,
}

/// Returns true if `r` is in `KNOWN_ROLES`.
pub fn is_valid_role(r: &str) -> bool {
    KNOWN_ROLES.iter().any(|&k| k == r)
}

/// Returns true if `c` is one of the 4 region chars.
pub fn is_valid_region(c: u8) -> bool {
    matches!(
        c,
        namespace::GRAND | namespace::HOST | namespace::FEDERATION | namespace::DATA
    )
}

/// Returns true if `s` is exactly 4 hex digits.
pub fn is_valid_host_code(s: &str) -> bool {
    s.len() == 4 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Returns true if `s` is exactly N hex digits (used for activity=2 and wave=3).
pub fn is_valid_hex_n(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_hexdigit())
}

// ===== 4-subclass taxonomy (per aether v2 invitation cycle 44; liris JS mirror sha=cbae3ec6d25a473d) =====
//
// Surface-reserved here per retraction-class clause 24: aether v2 module not yet read from acer
// vantage. Classification semantics finalize once aether v2 sha + signatures land on bus.
// Until then, classifier returns `Pending` for any input — does not falsely assert membership.

/// 5-subclass taxonomy per **aether v3 canonical mint** `:86485` (sha=`84a680d35b6091c4`).
///
/// Cycle-47 update: aether v3 supersedes the earlier hookwall-domain L1/L2 naming.
/// The 5 subclasses are FORM-based (PID-shape) not hookwall-domain (kernel vs userspace).
///
/// **Orthogonal DOMAIN axis (cycle-68):** Atlas at `D:/asolaria-whiteroom/behcs-1024-atlas/`
/// provides the canonical codepoint→domain map (1024 cp across 8 domains: legacy_subset_256,
/// sovereignty, sequencing, language_glyph, hilbert_cube, build_proof, federation,
/// emergent_residual). FORM axis classifies PID string shape; DOMAIN axis classifies cp
/// sovereignty role. See `kernel/docs/ATLAS_INTEGRATION.md`. `HookwallCp` classifier wires
/// to atlas when Phase-9 lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidSubclass {
    /// `<ROLE>-PID-<REGION><HOST>-A##-W###` — canonical short form, no suffix.
    Regular,
    /// `<ROLE>-PID-<REGION><HOST>-A##-W###-P##-N#####` — canonical with process+nonce suffix.
    /// All operator anchors (OP-JESSE, OP-RAYSSA) + acer resident anchors fall here.
    RegularExtended,
    /// `<ROLE>-PID-<YYYY-MM-DD>` — date-suffix anchor form. (Federation/project anchors.)
    Anchor,
    /// CP-prefix PIDs from Option-B regex / cycle-25 hookwall taxonomy.
    /// Concrete cp-range mapping pending aether v3 direct read on acer vantage.
    HookwallCp,
    /// Infrastructure-routing PIDs (5th subclass per aether v3).
    /// Concrete shape mapping pending aether v3 direct read on acer vantage.
    InfrastructureRouting,
    /// Pending classification — input shape doesn't match any of the 5 canonical forms,
    /// or aether v3 module not yet directly readable from acer vantage for HookwallCp /
    /// InfrastructureRouting concrete rules.
    Pending,
}

/// Returns the 5-subclass of a PID per aether v3 canonical taxonomy.
///
/// Cycle-47 dispatch (3 of 5 concretely wired):
/// - canonical extended form (`-P##-N#####`) → `RegularExtended`
/// - canonical short form (no suffix) → `Regular`
/// - date-suffix anchor form → `Anchor`
/// - cp-prefix and infrastructure-routing forms → `Pending` until aether v3 direct read
pub fn classify_subclass(pid: &str) -> PidSubclass {
    match parse_pid(pid) {
        Ok(ParsedPid::Canonical(p)) => {
            if p.process_tag.is_some() && p.nonce.is_some() {
                PidSubclass::RegularExtended
            } else {
                PidSubclass::Regular
            }
        }
        Ok(ParsedPid::Anchor(_)) => PidSubclass::Anchor,
        Err(_) => PidSubclass::Pending,
    }
}

/// Returns true if `pid` is in subclass `sub` per current classifier. False for `Pending`.
pub fn is_in_subclass(pid: &str, sub: PidSubclass) -> bool {
    classify_subclass(pid) == sub && sub != PidSubclass::Pending
}

/// Validates `pid` under the canonical 4-subclass rules. Returns Ok(()) when classified
/// (non-Pending) AND the underlying canonical/anchor parse succeeds. Errors otherwise.
pub fn validate_subclass(pid: &str) -> Result<PidSubclass, PidErr> {
    validate_pid(pid, false)?;
    let sub = classify_subclass(pid);
    Ok(sub)
}

/// Parsed PID fields (canonical format — anchor format uses [`AnchorPidParts`]).
///
/// Cycle-45 extension per liris ARGUS M4 finding `:86183`:
/// canonical form accepts optional `-P<2hex>-N<5hex>` suffix used by operator PIDs
/// (OP-JESSE-PID-G0000-A00-W000-P00-N00000, OP-RAYSSA-PID-G0000-A00-W000-P00-N00000)
/// and resident anchors (ACER-PID-H740C-A07-W104-P00-N00000). Both are now canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidParts {
    pub role: String,
    pub region: u8,
    pub host_code: String,
    pub activity: String,
    pub wave: String,
    /// Optional process tag (`P<2hex>`). `None` for short form.
    pub process_tag: Option<String>,
    /// Optional nonce tag (`N<5hex>`). `None` for short form.
    pub nonce: Option<String>,
}

/// Parsed anchor PID fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorPidParts {
    pub role: String,
    pub date: String, // YYYY-MM-DD
}

/// Discriminated result of `parse_pid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedPid {
    Canonical(PidParts),
    Anchor(AnchorPidParts),
}

/// Parses a PID string into either canonical or anchor form.
pub fn parse_pid(pid: &str) -> Result<ParsedPid, PidErr> {
    let idx = pid.rfind("-PID-").ok_or(PidErr::MalformedFormat)?;
    let role = &pid[..idx];
    let tail = &pid[idx + 5..]; // skip "-PID-"

    if tail.len() == 10
        && tail.as_bytes().get(4) == Some(&b'-')
        && tail.as_bytes().get(7) == Some(&b'-')
    {
        let date_ok = tail[..4].bytes().all(|b| b.is_ascii_digit())
            && tail[5..7].bytes().all(|b| b.is_ascii_digit())
            && tail[8..10].bytes().all(|b| b.is_ascii_digit());
        if date_ok {
            return Ok(ParsedPid::Anchor(AnchorPidParts {
                role: role.into(),
                date: tail.into(),
            }));
        }
    }

    let parts: Vec<&str> = tail.split('-').collect();
    // Short form: 3 parts (<REGION><HOST>-A##-W###)
    // Extended form (cycle-45): 5 parts (<REGION><HOST>-A##-W###-P##-N#####)
    if parts.len() != 3 && parts.len() != 5 {
        return Err(PidErr::MalformedFormat);
    }
    let region_host = parts[0];
    let activity_block = parts[1];
    let wave_block = parts[2];

    if region_host.len() != 5 {
        return Err(PidErr::MalformedFormat);
    }
    let region = region_host.as_bytes()[0];
    if !is_valid_region(region) {
        return Err(PidErr::InvalidRegion);
    }
    let host_code = &region_host[1..];
    if !is_valid_host_code(host_code) {
        return Err(PidErr::InvalidHostCode);
    }

    if !activity_block.starts_with('A') || !is_valid_hex_n(&activity_block[1..], 2) {
        return Err(PidErr::InvalidActivity);
    }
    if !wave_block.starts_with('W') || !is_valid_hex_n(&wave_block[1..], 3) {
        return Err(PidErr::InvalidWave);
    }

    let (process_tag, nonce) = if parts.len() == 5 {
        let process_block = parts[3];
        let nonce_block = parts[4];
        if !process_block.starts_with('P') || !is_valid_hex_n(&process_block[1..], 2) {
            return Err(PidErr::InvalidProcess);
        }
        if !nonce_block.starts_with('N') || !is_valid_hex_n(&nonce_block[1..], 5) {
            return Err(PidErr::InvalidNonce);
        }
        (
            Some(process_block[1..].into()),
            Some(nonce_block[1..].into()),
        )
    } else {
        (None, None)
    };

    Ok(ParsedPid::Canonical(PidParts {
        role: role.into(),
        region,
        host_code: host_code.into(),
        activity: activity_block[1..].into(),
        wave: wave_block[1..].into(),
        process_tag,
        nonce,
    }))
}

/// Validates a PID with the optional strict-role check.
pub fn validate_pid(pid: &str, strict_role: bool) -> Result<(), PidErr> {
    let parsed = parse_pid(pid)?;
    if strict_role {
        let role = match &parsed {
            ParsedPid::Canonical(p) => &p.role,
            ParsedPid::Anchor(p) => &p.role,
        };
        if !is_valid_role(role) {
            return Err(PidErr::UnknownRole);
        }
    }
    Ok(())
}

/// Mints a canonical PID (short form). For extended form with -P-N suffix
/// use [`mint_pid_extended`].
pub fn mint_pid(
    role: &str,
    region: u8,
    host_code: &str,
    activity: &str,
    wave: &str,
) -> Result<String, PidErr> {
    if !is_valid_region(region) {
        return Err(PidErr::InvalidRegion);
    }
    if !is_valid_host_code(host_code) {
        return Err(PidErr::InvalidHostCode);
    }
    if !is_valid_hex_n(activity, 2) {
        return Err(PidErr::InvalidActivity);
    }
    if !is_valid_hex_n(wave, 3) {
        return Err(PidErr::InvalidWave);
    }
    let mut s = String::new();
    s.push_str(role);
    s.push_str("-PID-");
    s.push(region as char);
    s.push_str(host_code);
    s.push_str("-A");
    s.push_str(activity);
    s.push_str("-W");
    s.push_str(wave);
    Ok(s)
}

/// Mints a canonical PID in extended form `<ROLE>-PID-<REGION><HOST>-A##-W###-P##-N#####`.
/// Cycle-45 addition per liris ARGUS M4 finding. Used by operator anchors and resident PIDs.
pub fn mint_pid_extended(
    role: &str,
    region: u8,
    host_code: &str,
    activity: &str,
    wave: &str,
    process_tag: &str,
    nonce: &str,
) -> Result<String, PidErr> {
    let mut s = mint_pid(role, region, host_code, activity, wave)?;
    if !is_valid_hex_n(process_tag, 2) {
        return Err(PidErr::InvalidProcess);
    }
    if !is_valid_hex_n(nonce, 5) {
        return Err(PidErr::InvalidNonce);
    }
    s.push_str("-P");
    s.push_str(process_tag);
    s.push_str("-N");
    s.push_str(nonce);
    Ok(s)
}

/// Mints an anchor PID (date-suffixed).
pub fn mint_anchor_pid(role: &str, date_yyyy_mm_dd: &str) -> Result<String, PidErr> {
    if date_yyyy_mm_dd.len() != 10 {
        return Err(PidErr::MalformedFormat);
    }
    let b = date_yyyy_mm_dd.as_bytes();
    if b[4] != b'-' || b[7] != b'-' {
        return Err(PidErr::MalformedFormat);
    }
    if !(b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit()))
    {
        return Err(PidErr::MalformedFormat);
    }
    let mut s = String::new();
    s.push_str(role);
    s.push_str("-PID-");
    s.push_str(date_yyyy_mm_dd);
    Ok(s)
}

/// Computes a 16-char hex fingerprint sha16 of a PID (first 16 chars of sha256 hex).
pub fn pid_fingerprint_sha16(pid: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(pid.as_bytes());
    let out = h.finalize();
    let mut s = String::new();
    for &b in &out[..8] {
        s.push_str(&hex_byte(b));
    }
    s
}

fn hex_byte(b: u8) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::new();
    s.push(HEX[(b >> 4) as usize] as char);
    s.push(HEX[(b & 0x0f) as usize] as char);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_roles_contains_canonical_set() {
        assert!(is_valid_role("OP-RAYSSA"));
        assert!(is_valid_role("OPERATOR"));
        assert!(is_valid_role("ACER"));
        assert!(is_valid_role("LIRIS"));
        assert!(is_valid_role("FALCON"));
        assert!(is_valid_role("AETHER"));
        assert!(!is_valid_role("RANDOM"));
    }

    #[test]
    fn region_validation() {
        for c in [b'G', b'H', b'F', b'D'] {
            assert!(is_valid_region(c));
        }
        assert!(!is_valid_region(b'X'));
    }

    #[test]
    fn historical_pid_op_rayssa_validates() {
        assert!(validate_pid("OP-RAYSSA-PID-G0000-A00-W000", true).is_ok());
    }

    #[test]
    fn historical_pid_operator_h1001_validates() {
        assert!(validate_pid("OPERATOR-PID-H1001-A00-W110", true).is_ok());
    }

    #[test]
    fn anchor_pid_asolaria_federation_remake_validates() {
        assert!(validate_pid("ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11", true).is_ok());
    }

    #[test]
    fn parse_canonical_roundtrip() {
        let p = "ACER-PID-H740C-A07-W104";
        let parsed = parse_pid(p).unwrap();
        match parsed {
            ParsedPid::Canonical(parts) => {
                assert_eq!(parts.role, "ACER");
                assert_eq!(parts.region, b'H');
                assert_eq!(parts.host_code, "740C");
                assert_eq!(parts.activity, "07");
                assert_eq!(parts.wave, "104");
                let minted = mint_pid(
                    &parts.role,
                    parts.region,
                    &parts.host_code,
                    &parts.activity,
                    &parts.wave,
                )
                .unwrap();
                assert_eq!(minted, p);
            }
            _ => panic!("expected Canonical"),
        }
    }

    #[test]
    fn parse_anchor_roundtrip() {
        let p = "ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11";
        let parsed = parse_pid(p).unwrap();
        match parsed {
            ParsedPid::Anchor(parts) => {
                assert_eq!(parts.role, "ASOLARIA-FEDERATION-REMAKE-1024");
                assert_eq!(parts.date, "2026-05-11");
                let minted = mint_anchor_pid(&parts.role, &parts.date).unwrap();
                assert_eq!(minted, p);
            }
            _ => panic!("expected Anchor"),
        }
    }

    #[test]
    fn invalid_region_rejected() {
        assert_eq!(
            parse_pid("ACER-PID-X740C-A07-W104"),
            Err(PidErr::InvalidRegion)
        );
    }

    #[test]
    fn fingerprint_is_16_chars_hex() {
        let fp = pid_fingerprint_sha16("ACER-PID-H740C-A07-W104");
        assert_eq!(fp.len(), 16);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn short_form_classifies_as_regular_per_aether_v3() {
        // Cycle-47 update: short form → Regular (not Pending) per aether v3 canonical naming.
        assert_eq!(
            classify_subclass("ACER-PID-H740C-A07-W104"),
            PidSubclass::Regular
        );
        assert_eq!(
            classify_subclass("OP-RAYSSA-PID-G0000-A00-W000"),
            PidSubclass::Regular
        );
        assert!(is_in_subclass(
            "ACER-PID-H740C-A07-W104",
            PidSubclass::Regular
        ));
    }

    #[test]
    fn anchor_form_classifies_as_anchor_per_aether_v3() {
        // Cycle-47 wire: date-suffix anchor → Anchor.
        assert_eq!(
            classify_subclass("ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11"),
            PidSubclass::Anchor
        );
    }

    #[test]
    fn malformed_input_still_pending() {
        assert_eq!(classify_subclass("not-a-pid"), PidSubclass::Pending);
    }

    #[test]
    fn extended_form_classifies_as_regular_extended_per_aether_86285() {
        // Cycle-46 wire: extended -P##-N##### form dispatches to RegularExtended.
        assert_eq!(
            classify_subclass("ACER-PID-H740C-A07-W104-P00-N00000"),
            PidSubclass::RegularExtended
        );
        assert_eq!(
            classify_subclass("OP-JESSE-PID-G0000-A00-W000-P00-N00000"),
            PidSubclass::RegularExtended
        );
        assert_eq!(
            classify_subclass("OP-RAYSSA-PID-G0000-A00-W000-P00-N00000"),
            PidSubclass::RegularExtended
        );
        // The 3 ARGUS M4 cycle-45 "drift" PIDs all classify as RegularExtended.
        assert_eq!(
            classify_subclass("ACER-PID-H3CC5-A02-W200-P00-N8e813"),
            PidSubclass::RegularExtended
        );
        assert_eq!(
            classify_subclass("ACER-PID-H7741-A02-W200-P00-N5e62a"),
            PidSubclass::RegularExtended
        );
    }

    #[test]
    fn is_in_subclass_true_for_extended_form_regular_extended() {
        assert!(is_in_subclass(
            "ACER-PID-H740C-A07-W104-P00-N00000",
            PidSubclass::RegularExtended
        ));
        assert!(!is_in_subclass(
            "ACER-PID-H740C-A07-W104-P00-N00000",
            PidSubclass::Regular
        ));
        assert!(!is_in_subclass(
            "ACER-PID-H740C-A07-W104-P00-N00000",
            PidSubclass::HookwallCp
        ));
    }

    #[test]
    fn validate_subclass_yields_regular_extended_on_extended_form() {
        let r = validate_subclass("ACER-PID-H740C-A07-W104-P00-N00000");
        assert!(matches!(r, Ok(PidSubclass::RegularExtended)));
    }

    #[test]
    fn validate_subclass_yields_regular_on_short_form() {
        let r = validate_subclass("ACER-PID-H740C-A07-W104");
        assert!(matches!(r, Ok(PidSubclass::Regular)));
    }

    #[test]
    fn validate_subclass_yields_anchor_on_date_form() {
        let r = validate_subclass("ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11");
        assert!(matches!(r, Ok(PidSubclass::Anchor)));
    }

    #[test]
    fn validate_subclass_rejects_malformed_input() {
        let r = validate_subclass("not-a-pid");
        assert!(r.is_err());
    }

    // ===== Cycle-45 extended-form tests per liris ARGUS M4 :86183 =====

    #[test]
    fn extended_form_acer_anchor_validates() {
        // Resident anchor used in every acer-side envelope since cycle-1.
        assert!(validate_pid("ACER-PID-H740C-A07-W104-P00-N00000", false).is_ok());
    }

    #[test]
    fn extended_form_op_jesse_anchor_validates() {
        assert!(validate_pid("OP-JESSE-PID-G0000-A00-W000-P00-N00000", true).is_ok());
    }

    #[test]
    fn extended_form_op_rayssa_anchor_validates() {
        assert!(validate_pid("OP-RAYSSA-PID-G0000-A00-W000-P00-N00000", true).is_ok());
    }

    #[test]
    fn extended_form_argus_m4_drift_samples_now_canonical() {
        // The 3 NEW drift PIDs liris ARGUS M4 flagged in :86183 are now canonical
        // under extended form. (Mixed-case nonce digits per ARGUS sample.)
        assert!(validate_pid("ACER-PID-H740C-A07-W104-P00-N00000", false).is_ok());
        assert!(validate_pid("ACER-PID-H3CC5-A02-W200-P00-N8e813", false).is_ok());
        assert!(validate_pid("ACER-PID-H7741-A02-W200-P00-N5e62a", false).is_ok());
    }

    #[test]
    fn extended_form_parse_roundtrip() {
        let p = "ACER-PID-H740C-A07-W104-P00-N00000";
        let parsed = parse_pid(p).unwrap();
        match parsed {
            ParsedPid::Canonical(parts) => {
                assert_eq!(parts.role, "ACER");
                assert_eq!(parts.region, b'H');
                assert_eq!(parts.host_code, "740C");
                assert_eq!(parts.activity, "07");
                assert_eq!(parts.wave, "104");
                assert_eq!(parts.process_tag.as_deref(), Some("00"));
                assert_eq!(parts.nonce.as_deref(), Some("00000"));
                let minted = mint_pid_extended(
                    &parts.role,
                    parts.region,
                    &parts.host_code,
                    &parts.activity,
                    &parts.wave,
                    parts.process_tag.as_deref().unwrap(),
                    parts.nonce.as_deref().unwrap(),
                )
                .unwrap();
                assert_eq!(minted, p);
            }
            _ => panic!("expected Canonical"),
        }
    }

    #[test]
    fn short_form_still_parses_with_none_extension_fields() {
        let p = "ACER-PID-H740C-A07-W104";
        let parsed = parse_pid(p).unwrap();
        match parsed {
            ParsedPid::Canonical(parts) => {
                assert_eq!(parts.process_tag, None);
                assert_eq!(parts.nonce, None);
            }
            _ => panic!("expected Canonical"),
        }
    }

    #[test]
    fn extended_form_rejects_invalid_process_tag() {
        // P-block must be P<2hex>; "PZZ" fails.
        assert_eq!(
            parse_pid("ACER-PID-H740C-A07-W104-PZZ-N00000"),
            Err(PidErr::InvalidProcess)
        );
    }

    #[test]
    fn extended_form_rejects_invalid_nonce() {
        // N-block must be N<5hex>; only 4 hex digits fails specific shape check
        // → InvalidNonce (more specific than MalformedFormat — parser reaches the N
        // validation since 5-part split succeeds).
        assert_eq!(
            parse_pid("ACER-PID-H740C-A07-W104-P00-N0000"),
            Err(PidErr::InvalidNonce)
        );
    }

    #[test]
    fn four_parts_still_malformed() {
        // 4 parts (no nonce) is rejected — extended form requires both P and N.
        assert_eq!(
            parse_pid("ACER-PID-H740C-A07-W104-P00"),
            Err(PidErr::MalformedFormat)
        );
    }
}
