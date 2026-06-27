//! mcp_health — claw-code 2.0's MCP degraded-mode, absorbed into the Host-8 frame (Rust, json=0).
//! claw-code degrades GRACEFULLY when an MCP connector is down/degraded and surfaces WHICH capability
//! was lost instead of failing opaquely or pretending all is well. Our frame: the live fabric (queried
//! via MCP/HBP) sometimes answers with a FALLBACK marker — an `HBPFALLBACK` envelope, a `*_fallback`
//! key, or a stale cached snapshot. CLAIMS-GATE LAW: a fallback-marked answer is SCOPED/degraded
//! evidence ONLY — it must NEVER be treated as "live". This module classifies each fabric/MCP lane
//! Healthy / Degraded / Down / Unknown, NAMES why, and makes the cardinal fake-clean sin — reporting a
//! fallback or stale answer as live/healthy — structurally impossible.
//!
//! CARDINAL INVARIANT: `is_live(d) == (d.health == Healthy) == (d.reason == None)`. Healthy is reached
//! on EXACTLY ONE cascade arm; fallback taint is checked BEFORE any freshness/healthy arm. Pure +
//! read-only: it CLASSIFIES — it never probes, reconnects, mutates, or fires.
//!
//! SAFE-DEFAULT INVERSION (deliberate, opposite of the optimistic sibling modules): every liveness-
//! gating field defaults to the PESSIMISTIC side (probed/transport_ok/responded/response_present=false,
//! age=None) so a bare/partial row can never reach Healthy — trust must be positively proven.

/// The trust verdict for one MCP/fabric lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneHealth {
    /// Live and trustworthy — the ONLY verdict that is live=true. Reached only on R8.
    Healthy,
    /// Reachable/answered but SCOPED (fallback-tainted, cached-while-down, or stale) — live=false.
    Degraded,
    /// No usable answer (transport down, server error, timeout, empty payload) — live=false.
    Down,
    /// Trust not established (not probed, in-flight, or contradictory) — live=false.
    Unknown,
}

impl LaneHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            LaneHealth::Healthy => "healthy",
            LaneHealth::Degraded => "degraded",
            LaneHealth::Down => "down",
            LaneHealth::Unknown => "unknown",
        }
    }
    /// Severity for the summary `worst=` field: down > degraded > unknown > healthy.
    fn severity(&self) -> u8 {
        match self {
            LaneHealth::Down => 3,
            LaneHealth::Degraded => 2,
            LaneHealth::Unknown => 1,
            LaneHealth::Healthy => 0,
        }
    }
}

/// The precise named cause — what (sub-)capability was lost and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradeReason {
    None,             // Healthy — the only reason that pairs with live=true
    FallbackEnvelope, // Degraded — HBPFALLBACK envelope tag
    FallbackKey,      // Degraded — a *_fallback-suffixed key in the payload
    CachedWhileDown,  // Degraded — payload served while transport_ok=false
    StaleSnapshot,    // Degraded — real but old (age >= stale_after)
    AgeUnknown,       // Degraded — freshness unprovable (age absent/future-dated)
    TransportDown,    // Down — connector/bus unreachable, no payload
    ServerError,      // Down — server returned an explicit error
    Timeout,          // Down — no response within the SLA
    EmptyResponse,    // Down — responded but payload empty/unusable
    NotProbed,        // Unknown — lane not probed this cycle
    InFlight,         // Unknown — probe pending inside its window
}

impl DegradeReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DegradeReason::None => "none",
            DegradeReason::FallbackEnvelope => "fallback_envelope",
            DegradeReason::FallbackKey => "fallback_key",
            DegradeReason::CachedWhileDown => "cached_while_down",
            DegradeReason::StaleSnapshot => "stale_snapshot",
            DegradeReason::AgeUnknown => "age_unknown",
            DegradeReason::TransportDown => "transport_down",
            DegradeReason::ServerError => "server_error",
            DegradeReason::Timeout => "timeout",
            DegradeReason::EmptyResponse => "empty_response",
            DegradeReason::NotProbed => "not_probed",
            DegradeReason::InFlight => "in_flight",
        }
    }
    /// True for FallbackEnvelope / FallbackKey — the cardinal claims-gate metric (`fallback_tainted=`).
    pub fn is_fallback_taint(&self) -> bool {
        matches!(
            self,
            DegradeReason::FallbackEnvelope | DegradeReason::FallbackKey
        )
    }
    /// True for StaleSnapshot / AgeUnknown — the freshness-degraded metric (`stale=`).
    pub fn is_stale(&self) -> bool {
        matches!(
            self,
            DegradeReason::StaleSnapshot | DegradeReason::AgeUnknown
        )
    }
}

/// Evidence about one lane this cycle. PESSIMISTIC defaults — trust is proven, not assumed.
#[derive(Debug, Clone)]
pub struct Evidence {
    pub lane: String,
    pub capability: String,
    pub probed: bool,
    pub transport_ok: bool,
    pub responded: bool,
    pub response_present: bool,
    pub server_error: String,
    pub fallback_marker: bool,
    pub fallback_key: bool,
    pub tainted_field: String,
    /// ms since the last successful non-fallback refresh. None = absent/future-dated => AgeUnknown.
    pub age_ms: Option<u64>,
    pub elapsed_ms: u64,
    /// The raw HBP response line; scanned by `detect_fallback` so a tag/key is caught even if the
    /// explicit bools were not set by the producer.
    pub raw: String,
}

/// The diagnosis for one lane.
#[derive(Debug, Clone)]
pub struct Diag {
    pub health: LaneHealth,
    pub reason: DegradeReason,
    pub tainted_field: String,
}

/// Scan a raw HBP line for a fallback signal. HBPFALLBACK envelope dominates a `*_fallback` key.
pub fn detect_fallback(raw: &str) -> Option<DegradeReason> {
    if raw.contains("HBPFALLBACK") {
        return Some(DegradeReason::FallbackEnvelope);
    }
    if has_fallback_key(raw) {
        return Some(DegradeReason::FallbackKey);
    }
    None
}

/// True if `raw` contains a token ending in `_fallback` (e.g. `supervisors_fallback`, or a bare
/// `_fallback`). One token-based path (`>=` includes the bare key) so the `=` form and the JSON `:`
/// form are both covered by splitting on non-(alphanumeric|`_`).
fn has_fallback_key(raw: &str) -> bool {
    raw.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|tok| tok.len() >= "_fallback".len() && tok.ends_with("_fallback"))
}

/// Derive the tainted sub-capability name (the prefix of a `*_fallback` token). A bare `_fallback`
/// (empty prefix) yields None so the caller falls back to "-".
fn derive_tainted(raw: &str) -> Option<String> {
    raw.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .find(|tok| tok.len() >= "_fallback".len() && tok.ends_with("_fallback"))
        .map(|tok| tok[..tok.len() - "_fallback".len()].to_string())
        .filter(|prefix| !prefix.is_empty())
}

/// Classify one lane. First-match-wins cascade, most-severe / most-load-bearing first. Healthy is
/// reachable on EXACTLY ONE arm (R8); fallback taint (R2/R3) is checked before any freshness/healthy
/// arm — the cardinal claims-gate guarantee. Pure: no I/O, no fire.
pub fn classify(e: &Evidence, stale_after_ms: u64, timeout_ms: u64) -> Diag {
    let none_field = || "-".to_string();

    // R1 — not probed: trust not established. (Also the bare-row landing spot: probed defaults false.)
    if !e.probed {
        return Diag {
            health: LaneHealth::Unknown,
            reason: DegradeReason::NotProbed,
            tainted_field: none_field(),
        };
    }

    if e.responded {
        // --- responded branch: fallback taint DOMINATES freshness/healthy ---
        let fb = detect_fallback(&e.raw); // scan raw once; reuse in R2 + R3
                                          // R2 — HBPFALLBACK envelope (explicit bool OR detected in raw).
        if e.fallback_marker || fb == Some(DegradeReason::FallbackEnvelope) {
            return Diag {
                health: LaneHealth::Degraded,
                reason: DegradeReason::FallbackEnvelope,
                tainted_field: if e.tainted_field != "-" {
                    e.tainted_field.clone()
                } else {
                    none_field()
                },
            };
        }
        // R3 — a *_fallback key (explicit bool OR detected in raw); name the lost sub-capability.
        if e.fallback_key || fb == Some(DegradeReason::FallbackKey) {
            let tainted = if e.tainted_field != "-" {
                e.tainted_field.clone()
            } else {
                derive_tainted(&e.raw).unwrap_or_else(none_field)
            };
            return Diag {
                health: LaneHealth::Degraded,
                reason: DegradeReason::FallbackKey,
                tainted_field: tainted,
            };
        }
        // R3.5 — an explicit server error on a RESPONSE is hard down-evidence; fail closed even
        // though the lane "responded" (an error response with a body still sets responded+present).
        // Without this guard a fresh, transport-up error response would fall through to R8/Healthy —
        // the server_error fail-open the adversarial review caught.
        if !e.server_error.is_empty() {
            return Diag {
                health: LaneHealth::Down,
                reason: DegradeReason::ServerError,
                tainted_field: none_field(),
            };
        }
        // R4 — responded but empty payload = capability lost.
        if !e.response_present {
            return Diag {
                health: LaneHealth::Down,
                reason: DegradeReason::EmptyResponse,
                tainted_field: none_field(),
            };
        }
        // R5 — payload served over a transport reported down = scoped cache, never live.
        if !e.transport_ok {
            return Diag {
                health: LaneHealth::Degraded,
                reason: DegradeReason::CachedWhileDown,
                tainted_field: none_field(),
            };
        }
        // R6 — freshness unprovable (absent / future-dated normalized to None) — fail-closed.
        let Some(age) = e.age_ms else {
            return Diag {
                health: LaneHealth::Degraded,
                reason: DegradeReason::AgeUnknown,
                tainted_field: none_field(),
            };
        };
        // R7 — stale (>= boundary: an answer exactly at the SLA is stale, not fresh).
        if age >= stale_after_ms {
            return Diag {
                health: LaneHealth::Degraded,
                reason: DegradeReason::StaleSnapshot,
                tainted_field: none_field(),
            };
        }
        // R8 — THE ONLY path to Healthy / live=true.
        return Diag {
            health: LaneHealth::Healthy,
            reason: DegradeReason::None,
            tainted_field: none_field(),
        };
    }

    // --- no-response branch: probed && !responded ---
    // R9 — no payload AND transport down.
    if !e.transport_ok {
        return Diag {
            health: LaneHealth::Down,
            reason: DegradeReason::TransportDown,
            tainted_field: none_field(),
        };
    }
    // R10 — transport up but an explicit server error.
    if !e.server_error.is_empty() {
        return Diag {
            health: LaneHealth::Down,
            reason: DegradeReason::ServerError,
            tainted_field: none_field(),
        };
    }
    // R11 — timed out with no response.
    if e.elapsed_ms >= timeout_ms {
        return Diag {
            health: LaneHealth::Down,
            reason: DegradeReason::Timeout,
            tainted_field: none_field(),
        };
    }
    // R12 — probe pending inside its window: not Down, not Healthy.
    Diag {
        health: LaneHealth::Unknown,
        reason: DegradeReason::InFlight,
        tainted_field: none_field(),
    }
}

/// The single consumer-facing predicate encoding the CLAIMS-GATE law: may this lane's fabric answer be
/// treated as live? True IFF Healthy.
pub fn is_live(d: &Diag) -> bool {
    d.health == LaneHealth::Healthy
}

/// The most-severe verdict present across a fleet, for the summary `worst=` field. Empty => "healthy".
pub fn worst(diags: &[Diag]) -> LaneHealth {
    diags
        .iter()
        .map(|d| d.health)
        .max_by_key(|h| h.severity())
        .unwrap_or(LaneHealth::Healthy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_evidence() -> Evidence {
        // a fully-proven-live lane; tests flip exactly one field to probe each arm.
        Evidence {
            lane: "asolaria-fabric".to_string(),
            capability: "council_query".to_string(),
            probed: true,
            transport_ok: true,
            responded: true,
            response_present: true,
            server_error: String::new(),
            fallback_marker: false,
            fallback_key: false,
            tainted_field: "-".to_string(),
            age_ms: Some(1000),
            elapsed_ms: 0,
            raw: String::new(),
        }
    }

    #[test]
    fn classify_healthy_path() {
        let d = classify(&live_evidence(), 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Healthy);
        assert_eq!(d.reason, DegradeReason::None);
        assert!(is_live(&d));
    }

    #[test]
    fn classify_fresh_fallback_envelope_is_degraded_not_healthy() {
        // CARDINAL: a fresh, transport-up, structurally-complete answer that carries HBPFALLBACK is
        // STILL degraded — taint dominates freshness.
        let mut e = live_evidence();
        e.fallback_marker = true;
        e.age_ms = Some(10);
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Degraded);
        assert_eq!(d.reason, DegradeReason::FallbackEnvelope);
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_buried_fallback_key_sets_tainted_field() {
        let mut e = live_evidence();
        e.fallback_key = true;
        e.tainted_field = "supervisors".to_string();
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::FallbackKey);
        assert_eq!(d.tainted_field, "supervisors");
        assert!(!is_live(&d));
    }

    #[test]
    fn detect_fallback_from_raw_envelope() {
        let mut e = live_evidence();
        e.fallback_marker = false;
        e.raw = "SUPERVISORS|HBPFALLBACK|count=300|json=0".to_string();
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::FallbackEnvelope);
    }

    #[test]
    fn detect_fallback_from_raw_key_only_derives_tainted() {
        let mut e = live_evidence();
        e.raw = "SUPERVISORS|supervisors_fallback=stale|json=0".to_string();
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::FallbackKey);
        assert_eq!(d.tainted_field, "supervisors");
    }

    #[test]
    fn classify_cached_while_down_is_degraded() {
        let mut e = live_evidence();
        e.transport_ok = false;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Degraded);
        assert_eq!(d.reason, DegradeReason::CachedWhileDown);
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_responded_but_empty_is_down() {
        let mut e = live_evidence();
        e.response_present = false;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Down);
        assert_eq!(d.reason, DegradeReason::EmptyResponse);
    }

    #[test]
    fn classify_stale_at_boundary_is_degraded() {
        let mut e = live_evidence();
        e.age_ms = Some(30_000);
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::StaleSnapshot);
    }

    #[test]
    fn classify_fresh_one_below_boundary_is_healthy() {
        let mut e = live_evidence();
        e.age_ms = Some(29_999);
        assert_eq!(classify(&e, 30_000, 10_000).health, LaneHealth::Healthy);
    }

    #[test]
    fn classify_age_absent_is_age_unknown_fail_closed() {
        let mut e = live_evidence();
        e.age_ms = None;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::AgeUnknown);
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_not_probed_is_unknown() {
        let mut e = live_evidence();
        e.probed = false;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Unknown);
        assert_eq!(d.reason, DegradeReason::NotProbed);
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_transport_down_no_response_is_down() {
        let mut e = live_evidence();
        e.responded = false;
        e.transport_ok = false;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::TransportDown);
    }

    #[test]
    fn classify_server_error_is_down() {
        let mut e = live_evidence();
        e.responded = false;
        e.server_error = "ECONNREFUSED".to_string();
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::ServerError);
    }

    #[test]
    fn classify_responded_error_response_is_down_not_healthy() {
        // ADVERSARIAL-REVIEW FIX: an error RESPONSE (responded + present + transport_ok + fresh) that
        // also carries server_error must be Down/ServerError, never Healthy/live=true (the fail-open).
        let mut e = live_evidence();
        e.server_error = "HTTP 500 internal".to_string();
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Down);
        assert_eq!(d.reason, DegradeReason::ServerError);
        assert!(!is_live(&d));
    }

    #[test]
    fn detect_bare_fallback_key_is_degraded_not_healthy() {
        // ADVERSARIAL-REVIEW FIX: a bare `_fallback` token (len == 9, prefix empty) is still taint;
        // it must not slip through to Healthy. tainted_field falls back to "-" (no sub-capability).
        let mut e = live_evidence();
        e.raw = "SUP|_fallback=stale|json=0".to_string();
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::FallbackKey);
        assert_eq!(d.tainted_field, "-");
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_timeout_is_down() {
        let mut e = live_evidence();
        e.responded = false;
        e.elapsed_ms = 10_000;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::Timeout);
    }

    #[test]
    fn classify_in_flight_within_sla_is_unknown() {
        let mut e = live_evidence();
        e.responded = false;
        e.elapsed_ms = 5_000;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.health, LaneHealth::Unknown);
        assert_eq!(d.reason, DegradeReason::InFlight);
    }

    #[test]
    fn classify_conflict_taint_dominates_cached() {
        // responded + present but transport down AND fallback marker -> taint (R2) wins over R5.
        let mut e = live_evidence();
        e.transport_ok = false;
        e.fallback_marker = true;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::FallbackEnvelope);
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_conflict_no_response_down_over_taint() {
        // no payload -> the taint rules (which require responded) do not apply; hard-down wins.
        let mut e = live_evidence();
        e.responded = false;
        e.transport_ok = false;
        e.fallback_marker = true;
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::TransportDown);
    }

    #[test]
    fn classify_multiple_degraded_reports_most_specific() {
        let mut e = live_evidence();
        e.fallback_marker = true;
        e.fallback_key = true;
        e.age_ms = Some(99_999);
        let d = classify(&e, 30_000, 10_000);
        assert_eq!(d.reason, DegradeReason::FallbackEnvelope);
        assert!(!is_live(&d));
    }

    #[test]
    fn classify_ttl_zero_over_degrades_never_live() {
        let mut e = live_evidence();
        e.age_ms = Some(1);
        let d = classify(&e, 0, 10_000); // stale_after=0 -> any nonzero age is stale
        assert_eq!(d.reason, DegradeReason::StaleSnapshot);
        assert!(!is_live(&d));
    }

    #[test]
    fn invariant_live_iff_healthy_iff_reason_none() {
        // Property: across a matrix of evidence permutations, live == Healthy == reason==None.
        for &probed in &[false, true] {
            for &transport_ok in &[false, true] {
                for &responded in &[false, true] {
                    for &response_present in &[false, true] {
                        for &fallback_marker in &[false, true] {
                            for &age in &[None, Some(10u64), Some(99_999u64)] {
                                let e = Evidence {
                                    lane: "x".to_string(),
                                    capability: "c".to_string(),
                                    probed,
                                    transport_ok,
                                    responded,
                                    response_present,
                                    server_error: String::new(),
                                    fallback_marker,
                                    fallback_key: false,
                                    tainted_field: "-".to_string(),
                                    age_ms: age,
                                    elapsed_ms: 0,
                                    raw: String::new(),
                                };
                                let d = classify(&e, 30_000, 10_000);
                                let live = is_live(&d);
                                assert_eq!(live, d.health == LaneHealth::Healthy);
                                assert_eq!(live, d.reason == DegradeReason::None);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn worst_picks_most_severe() {
        let down = Diag {
            health: LaneHealth::Down,
            reason: DegradeReason::Timeout,
            tainted_field: "-".to_string(),
        };
        let healthy = Diag {
            health: LaneHealth::Healthy,
            reason: DegradeReason::None,
            tainted_field: "-".to_string(),
        };
        assert_eq!(worst(&[healthy.clone(), down]).as_str(), "down");
        assert_eq!(worst(&[healthy]).as_str(), "healthy");
        assert_eq!(worst(&[]).as_str(), "healthy");
    }
}
