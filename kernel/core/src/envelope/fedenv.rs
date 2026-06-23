//! FEDENV-v1 application-envelope contract — the omnidispatcher route-layer validator + target
//! resolution, ported byte-faithfully from `tools/omnidispatcher/{validator,routes}.mjs`.
//!
//! This is the ROUTE LAYER that sits IN FRONT of the agent registry (per the corrected wiring
//! order): a FEDENV-v1 envelope is validated + its target resolved to a `RouteKind` BEFORE any
//! handle-table / dispatch work. This module is PURE and E=0 — it validates and classifies; it
//! does NOT execute a route (no process launch, no socket, no file). The actual downstream
//! dispatch (multi-cli-invoke / bus-direct / citizen-stub / …) is the gated launch lane (STEP 5+),
//! kept in host8-serve behind `&fire=1` + EXEC-FREEZE release. `tools/omnidispatcher` remains the
//! node-vs-Rust PARITY ORACLE: identical envelopes must yield identical validate/resolve verdicts.
//!
//! Spec (validator.mjs): 11 required fields, 7 target prefixes, 64 KB payload cap, cube_47d = six
//! 0-7 ints, glyph_5 >= 5 glyphs, ttl in (0, 86400], row_hash/antecedents = 16 lowercase hex,
//! cosign_token must carry a valid window, and (if `ts` present) row_hash self-verify against
//! sha256("FEDENV|"+caller_pid+verb+payload+ts)[:16].

use sha2::{Digest, Sha256};

/// Legacy 2-week delegated cosign window (validator.mjs COSIGN_WINDOW).
pub const COSIGN_WINDOW: &str = "QUINTUPLE-DELEGATED-2WEEK-2026-05-22-to-2026-06-05";
/// Foundation-v3 LAW extension window (validator.mjs COSIGN_WINDOW_V3, -> 2026-09-23).
pub const COSIGN_WINDOW_V3: &str = "FOUNDATION-V3-LAW-EXTENDED-4MO";
/// Apex admin override token (validator.mjs ADMIN_OVERRIDE).
pub const ADMIN_OVERRIDE: &str = "ADMIN-OVERRIDE-OP-JESSE";
/// Payload cap (64 KB), matching validator.mjs MAX_PAYLOAD_BYTES.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Known target prefixes (validator.mjs TARGET_PREFIXES). Order is the resolver precedence.
pub const TARGET_PREFIXES: [&str; 7] = [
    "pid:H",
    "cli:",
    "citizen:",
    "antigravity:",
    "daemon:",
    "google:",
    "meta:",
];

/// Rejection reasons — the `EVT-FEDENV-REJECTED-*` taxonomy from validator.mjs (+ routes.mjs
/// reserved-slot reject). `as_event_str` renders the exact wire string the node dispatcher emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Missing/empty required field, or a structural shape violation (cube/glyph/ttl/hex).
    Malformed,
    /// `target` does not begin with any known resolver prefix.
    UnresolvableTarget,
    /// `payload` exceeds `MAX_PAYLOAD_BYTES`.
    PayloadTooLarge,
    /// `cosign_token` carries no valid window (2WK / V3-4MO / ADMIN-OVERRIDE).
    ExpiredCosign,
    /// `ts` present and `row_hash` != sha256("FEDENV|"+caller_pid+verb+payload+ts)[:16].
    RowHashMismatch,
    /// Target resolved to a reserved slot (fractal sub-spawn lane) — routes.mjs routeReserved.
    ReservedSlot,
}

impl RejectReason {
    /// The exact `EVT-FEDENV-REJECTED-*` event string emitted by the node dispatcher.
    pub fn as_event_str(self) -> &'static str {
        match self {
            RejectReason::Malformed => "EVT-FEDENV-REJECTED-MALFORMED",
            RejectReason::UnresolvableTarget => "EVT-FEDENV-REJECTED-UNRESOLVABLE-TARGET",
            RejectReason::PayloadTooLarge => "EVT-FEDENV-REJECTED-PAYLOAD-TOO-LARGE",
            RejectReason::ExpiredCosign => "EVT-FEDENV-REJECTED-EXPIRED-COSIGN",
            RejectReason::RowHashMismatch => "EVT-FEDENV-REJECTED-ROW-HASH-MISMATCH",
            RejectReason::ReservedSlot => "EVT-FEDENV-REJECTED-RESERVED-SLOT",
        }
    }
}

/// Resolved route kind from the `target` prefix grammar (routes.mjs resolveTarget / ROUTE_TABLE).
/// Classification only — NOT execution. The downstream verb (multi-cli-invoke / bus-direct / …)
/// is wired in the gated launch lane, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    /// `pid:H<coord>` — routed to the local 8-byte/PID handle table (the agent registry).
    PidRouted,
    /// `cli:<role>:<model>` — the multi-cli-invoke (opencode/Hermes) $0 lane.
    Cli,
    /// `citizen:<vantage>` — citizen-stub-queue (inbox write).
    Citizen,
    /// `antigravity:<model>` — omniscrcpy-antigravity-proxy.
    Antigravity,
    /// `daemon:<entity>` — bus-direct (POST to bus :4947).
    Daemon,
    /// `google:` — google-api-client (STUB — cloud-gated).
    Google,
    /// `meta:` — meta-supervisor-slot (queued).
    Meta,
}

/// Priority lane (validator.mjs priorityOf). Defaults to `Normal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Apex,
    High,
    Normal,
    Low,
}

/// A borrowed view of a FEDENV-v1 envelope's fields. Caller parses the wire format (HBP/JSON) into
/// this; an absent field is represented as `""` (mirrors node's `=== '' / undefined` check). `ts`
/// is the optional strict-mode field that triggers row_hash self-verify.
#[derive(Debug, Clone, Copy)]
pub struct FedenvView<'a> {
    pub caller_pid: &'a str,
    pub target: &'a str,
    pub verb: &'a str,
    pub payload: &'a str,
    pub back_address: &'a str,
    pub cube_47d: &'a str,
    pub glyph_5: &'a str,
    pub cosign_token: &'a str,
    pub ttl_seconds: &'a str,
    pub antecedents: &'a str,
    pub row_hash: &'a str,
    pub ts: Option<&'a str>,
}

/// Validate a FEDENV-v1 envelope. `Ok(())` = accept; `Err(reason)` = reject with the taxonomy
/// reason. Rule order mirrors validator.mjs exactly so node and Rust verdicts are byte-equal.
pub fn validate(env: &FedenvView<'_>) -> Result<(), RejectReason> {
    // 1. required fields present + non-empty.
    for f in &[
        env.caller_pid,
        env.target,
        env.verb,
        env.payload,
        env.back_address,
        env.cube_47d,
        env.glyph_5,
        env.cosign_token,
        env.ttl_seconds,
        env.antecedents,
        env.row_hash,
    ] {
        if f.is_empty() {
            return Err(RejectReason::Malformed);
        }
    }
    // 2. target prefix must be a known resolver.
    if !TARGET_PREFIXES.iter().any(|p| env.target.starts_with(p)) {
        return Err(RejectReason::UnresolvableTarget);
    }
    // 3. payload size cap (utf8 bytes).
    if env.payload.len() > MAX_PAYLOAD_BYTES {
        return Err(RejectReason::PayloadTooLarge);
    }
    // 4. cube_47d: six 0-7 ints, hyphen-joined.
    if !cube_47d_ok(env.cube_47d) {
        return Err(RejectReason::Malformed);
    }
    // 5. glyph_5: >= 5 glyphs.
    if env.glyph_5.chars().count() < 5 {
        return Err(RejectReason::Malformed);
    }
    // 6. ttl_seconds: finite, (0, 86400].
    match env.ttl_seconds.parse::<f64>() {
        Ok(t) if t.is_finite() && t > 0.0 && t <= 86400.0 => {}
        _ => return Err(RejectReason::Malformed),
    }
    // 7/8. row_hash + antecedents: 16 lowercase hex.
    if !is_hex16(env.row_hash) || !is_hex16(env.antecedents) {
        return Err(RejectReason::Malformed);
    }
    // 9. cosign window.
    if !(env.cosign_token.contains(COSIGN_WINDOW)
        || env.cosign_token.contains(COSIGN_WINDOW_V3)
        || env.cosign_token.contains(ADMIN_OVERRIDE))
    {
        return Err(RejectReason::ExpiredCosign);
    }
    // 10. row_hash self-verify (best-effort, only when strict-mode `ts` is present).
    if let Some(ts) = env.ts {
        let mut h = Sha256::new();
        h.update(b"FEDENV|");
        h.update(env.caller_pid.as_bytes());
        h.update(env.verb.as_bytes());
        h.update(env.payload.as_bytes());
        h.update(ts.as_bytes());
        let out = h.finalize();
        if !row_hash_matches(&out[0..8], env.row_hash) {
            return Err(RejectReason::RowHashMismatch);
        }
    }
    Ok(())
}

/// Resolve a `target` to its `RouteKind` by prefix (routes.mjs resolveTarget grammar). Returns
/// `None` for an unresolvable prefix (validator catches that first as `UnresolvableTarget`).
pub fn resolve_target(target: &str) -> Option<RouteKind> {
    if target.starts_with("pid:H") {
        Some(RouteKind::PidRouted)
    } else if target.starts_with("cli:") {
        Some(RouteKind::Cli)
    } else if target.starts_with("citizen:") {
        Some(RouteKind::Citizen)
    } else if target.starts_with("antigravity:") {
        Some(RouteKind::Antigravity)
    } else if target.starts_with("daemon:") {
        Some(RouteKind::Daemon)
    } else if target.starts_with("google:") {
        Some(RouteKind::Google)
    } else if target.starts_with("meta:") {
        Some(RouteKind::Meta)
    } else {
        None
    }
}

/// Derive a priority lane from the `priority` field (validator.mjs priorityOf). Default `Normal`.
pub fn priority_of(priority: &str) -> Priority {
    match priority {
        "apex" => Priority::Apex,
        "high" => Priority::High,
        "low" => Priority::Low,
        _ => Priority::Normal,
    }
}

/// `true` iff `s` is exactly 16 lowercase-hex chars.
fn is_hex16(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `true` iff `s` is six `0..=7` single-digit segments joined by `-` (e.g. "1-2-3-4-5-6").
fn cube_47d_ok(s: &str) -> bool {
    let mut segments = 0u32;
    for seg in s.split('-') {
        segments += 1;
        if seg.len() != 1 || !matches!(seg.as_bytes()[0], b'0'..=b'7') {
            return false;
        }
    }
    segments == 6
}

/// Compare the first-8-bytes of a sha256 digest (rendered lowercase hex) to a 16-hex string,
/// without allocating. `expected` is assumed already shape-checked (`is_hex16`).
fn row_hash_matches(digest_first8: &[u8], expected: &str) -> bool {
    if expected.len() != 16 || digest_first8.len() < 8 {
        return false;
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let eb = expected.as_bytes();
    for i in 0..8 {
        let hi = HEX[(digest_first8[i] >> 4) as usize];
        let lo = HEX[(digest_first8[i] & 0x0f) as usize];
        if eb[2 * i] != hi || eb[2 * i + 1] != lo {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid<'a>() -> FedenvView<'a> {
        FedenvView {
            caller_pid: "AGT-ACER-HRM-HABCDEF012345",
            target: "cli:engineering:opencode/big-pickle",
            verb: "EVT-DISPATCH",
            payload: "hello world",
            back_address: "pid:H740C",
            cube_47d: "1-2-3-4-5-6",
            glyph_5: "ABCDE",
            cosign_token: "tok|FOUNDATION-V3-LAW-EXTENDED-4MO",
            ttl_seconds: "300",
            antecedents: "0000000000000000",
            row_hash: "0123456789abcdef",
            ts: None,
        }
    }

    #[test]
    fn valid_envelope_passes() {
        assert_eq!(validate(&valid()), Ok(()));
    }

    #[test]
    fn missing_field_is_malformed() {
        let mut e = valid();
        e.verb = "";
        assert_eq!(validate(&e), Err(RejectReason::Malformed));
    }

    #[test]
    fn unknown_prefix_is_unresolvable() {
        let mut e = valid();
        e.target = "ftp://nope";
        assert_eq!(validate(&e), Err(RejectReason::UnresolvableTarget));
    }

    #[test]
    fn bad_cube_is_malformed() {
        let mut e = valid();
        e.cube_47d = "1-2-3-4-5"; // only 5 segments
        assert_eq!(validate(&e), Err(RejectReason::Malformed));
        let mut e2 = valid();
        e2.cube_47d = "1-2-3-4-5-9"; // 9 out of 0-7 range
        assert_eq!(validate(&e2), Err(RejectReason::Malformed));
    }

    #[test]
    fn short_glyph_is_malformed() {
        let mut e = valid();
        e.glyph_5 = "ABC";
        assert_eq!(validate(&e), Err(RejectReason::Malformed));
    }

    #[test]
    fn ttl_out_of_range_is_malformed() {
        let mut e = valid();
        e.ttl_seconds = "0";
        assert_eq!(validate(&e), Err(RejectReason::Malformed));
        let mut e2 = valid();
        e2.ttl_seconds = "100000"; // > 86400
        assert_eq!(validate(&e2), Err(RejectReason::Malformed));
        let mut e3 = valid();
        e3.ttl_seconds = "abc";
        assert_eq!(validate(&e3), Err(RejectReason::Malformed));
    }

    #[test]
    fn bad_hex_fields_are_malformed() {
        let mut e = valid();
        e.row_hash = "XYZ";
        assert_eq!(validate(&e), Err(RejectReason::Malformed));
        let mut e2 = valid();
        e2.antecedents = "0123456789ABCDEF"; // uppercase not allowed
        assert_eq!(validate(&e2), Err(RejectReason::Malformed));
    }

    #[test]
    fn missing_cosign_window_is_expired() {
        let mut e = valid();
        e.cosign_token = "tok|NO-VALID-WINDOW";
        assert_eq!(validate(&e), Err(RejectReason::ExpiredCosign));
    }

    #[test]
    fn admin_override_accepted() {
        let mut e = valid();
        e.cosign_token = "ADMIN-OVERRIDE-OP-JESSE";
        assert_eq!(validate(&e), Ok(()));
    }

    #[test]
    fn resolve_targets_by_prefix() {
        assert_eq!(resolve_target("cli:eng:opencode"), Some(RouteKind::Cli));
        assert_eq!(resolve_target("pid:H740C"), Some(RouteKind::PidRouted));
        assert_eq!(resolve_target("citizen:liris"), Some(RouteKind::Citizen));
        assert_eq!(
            resolve_target("antigravity:gpt"),
            Some(RouteKind::Antigravity)
        );
        assert_eq!(resolve_target("daemon:gaia"), Some(RouteKind::Daemon));
        assert_eq!(resolve_target("google:bigquery"), Some(RouteKind::Google));
        assert_eq!(resolve_target("meta:sup"), Some(RouteKind::Meta));
        assert_eq!(resolve_target("ftp://x"), None);
    }

    #[test]
    fn priority_default_is_normal() {
        assert_eq!(priority_of("apex"), Priority::Apex);
        assert_eq!(priority_of("high"), Priority::High);
        assert_eq!(priority_of("low"), Priority::Low);
        assert_eq!(priority_of("garbage"), Priority::Normal);
    }

    #[test]
    fn reject_reason_event_strings() {
        assert_eq!(
            RejectReason::Malformed.as_event_str(),
            "EVT-FEDENV-REJECTED-MALFORMED"
        );
        assert_eq!(
            RejectReason::UnresolvableTarget.as_event_str(),
            "EVT-FEDENV-REJECTED-UNRESOLVABLE-TARGET"
        );
        assert_eq!(
            RejectReason::RowHashMismatch.as_event_str(),
            "EVT-FEDENV-REJECTED-ROW-HASH-MISMATCH"
        );
    }

    #[test]
    fn row_hash_self_verify_pass_and_fail() {
        // Strict-mode envelope with ts present: compute the correct row_hash, assert pass.
        let ts = "2026-06-22T00:00:00Z";
        let caller = "AGT-ACER-HRM-HABCDEF012345";
        let verb = "EVT-DISPATCH";
        let payload = "hello world";
        let mut h = Sha256::new();
        h.update(b"FEDENV|");
        h.update(caller.as_bytes());
        h.update(verb.as_bytes());
        h.update(payload.as_bytes());
        h.update(ts.as_bytes());
        let out = h.finalize();
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut hex = [0u8; 16];
        for i in 0..8 {
            hex[2 * i] = HEX[(out[i] >> 4) as usize];
            hex[2 * i + 1] = HEX[(out[i] & 0x0f) as usize];
        }
        let hexstr = core::str::from_utf8(&hex).unwrap();
        let mut e = valid();
        e.caller_pid = caller;
        e.verb = verb;
        e.payload = payload;
        e.ts = Some(ts);
        e.row_hash = hexstr;
        assert_eq!(
            validate(&e),
            Ok(()),
            "correct row_hash with ts present must pass"
        );
        // Tamper: a valid-shape but wrong row_hash with ts present must be rejected.
        e.row_hash = "ffffffffffffffff";
        assert_eq!(validate(&e), Err(RejectReason::RowHashMismatch));
    }
}
