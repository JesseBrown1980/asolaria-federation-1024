//! CYCLE-ORCHESTRATOR · BEHCS-256 absorb (selective Rust port)
//!
//! Sources (BEHCS-256 legacy at `C:/asolaria-behcs-256/packages/cycle-orchestrator/src/`):
//!   - `peer-state-machine.mjs` (UPGRADE-1) — 7-state FSM with legal transition table
//!   - `slo-gate.mjs` (UPGRADE-5) — halt-predicate evaluator (U-006..U-014)
//!
//! Per Vanguard scout #2 verdict: port state machines + SLO gate to Rust.
//! Defer main-loop wiring (`index.mjs`) to v0.2 pending deterministic test harness.

use alloc::collections::VecDeque;
use alloc::string::String;

// ============================================================================
// PEER STATE MACHINE (UPGRADE-1)
// ============================================================================

/// 7-state peer lifecycle per `peer-state-machine.mjs` lines 8-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// No signal; not responding.
    Dark,
    /// Health probe in flight.
    Probing,
    /// Keyboard daemon up; bus reachable.
    Alive,
    /// Physical kick typed; waiting on reply.
    Kicked,
    /// Reply verb observed within deadline.
    Responding,
    /// Reply diverges from expected (bilateral seal broke).
    Drift,
    /// U-006/U-007/U-008/U-010 predicate fired.
    Halt,
}

/// Returns the set of legal next-states from a given current state.
pub fn legal_transitions(from: PeerState) -> &'static [PeerState] {
    use PeerState::*;
    match from {
        Dark => &[Probing],
        Probing => &[Alive, Dark],
        Alive => &[Kicked, Dark, Probing],
        Kicked => &[Responding, Drift, Halt, Dark],
        Responding => &[Alive, Kicked, Drift, Halt],
        Drift => &[Alive, Halt, Dark],
        Halt => &[Dark],
    }
}

/// Transition history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionRecord {
    pub ts_ns: u64,
    pub from: PeerState,
    pub to: PeerState,
    pub reason: String,
}

/// Per-peer state machine.
pub struct PeerStateMachine {
    pub peer_id: String,
    pub state: PeerState,
    pub history: VecDeque<TransitionRecord>,
    pub last_transition_ns: u64,
    pub kick_deadline_ns: Option<u64>,
    pub strikes: u32,
}

impl PeerStateMachine {
    pub fn new(peer_id: &str) -> Self {
        Self {
            peer_id: peer_id.into(),
            state: PeerState::Dark,
            history: VecDeque::new(),
            last_transition_ns: 0,
            kick_deadline_ns: None,
            strikes: 0,
        }
    }

    /// Attempt a transition. Returns Err if illegal.
    pub fn transition(
        &mut self,
        next: PeerState,
        reason: &str,
        now_ns: u64,
    ) -> Result<(), &'static str> {
        let legal = legal_transitions(self.state);
        if !legal.contains(&next) {
            return Err("illegal_transition");
        }
        let prev = self.state;
        self.state = next;
        self.last_transition_ns = now_ns;
        self.history.push_back(TransitionRecord {
            ts_ns: now_ns,
            from: prev,
            to: next,
            reason: reason.into(),
        });
        if self.history.len() > 256 {
            self.history.pop_front();
        }
        Ok(())
    }

    pub fn on_kick(
        &mut self,
        _kick_text: &str,
        deadline_ns_from_now: u64,
        now_ns: u64,
    ) -> Result<(), &'static str> {
        self.kick_deadline_ns = Some(now_ns + deadline_ns_from_now);
        self.transition(PeerState::Kicked, "kick-issued", now_ns)
    }

    pub fn on_reply(&mut self, _reply_verb: &str, now_ns: u64) -> Result<(), &'static str> {
        self.strikes = 0;
        self.transition(PeerState::Responding, "reply-observed", now_ns)
    }

    pub fn on_drift(&mut self, _diff_summary: &str, now_ns: u64) -> Result<(), &'static str> {
        self.strikes += 1;
        self.transition(PeerState::Drift, "drift-detected", now_ns)
    }

    pub fn on_halt(&mut self, _predicate: &str, now_ns: u64) -> Result<(), &'static str> {
        self.transition(PeerState::Halt, "halt-predicate-fired", now_ns)
    }

    pub fn is_overdue(&self, now_ns: u64) -> bool {
        match (self.state, self.kick_deadline_ns) {
            (PeerState::Kicked, Some(dl)) => now_ns > dl,
            _ => false,
        }
    }
}

// ============================================================================
// SLO GATE (UPGRADE-5) — halt predicates
// ============================================================================

/// Default SLO thresholds per JS source lines 22-35.
pub struct SloThresholds {
    pub mem_free_mb_floor: u32,
    pub mem_duration_ms_floor: u32,
    /// Error-rate halt threshold in PERMILLE (integer only; 100 = 10%).
    pub err_rate_threshold_permille: u16,
    pub err_window_ms: u32,
    pub err_min_samples: u32,
    pub law_window_ms: u32,
    pub law_threshold: u32,
    pub flap_window_ms: u32,
    pub flap_threshold: u32,
    pub cadence_floor_ms: u32,
    pub state_file_stale_ms: u32,
}

impl Default for SloThresholds {
    fn default() -> Self {
        Self {
            mem_free_mb_floor: 200,
            mem_duration_ms_floor: 30_000,
            err_rate_threshold_permille: 100,
            err_window_ms: 60_000,
            err_min_samples: 5,
            law_window_ms: 60_000,
            law_threshold: 3,
            flap_window_ms: 30_000,
            flap_threshold: 5,
            cadence_floor_ms: 5_000,
            state_file_stale_ms: 120_000,
        }
    }
}

/// Halt-predicate identifiers per JS source comment block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltPredicate {
    /// U-006: mem_free_mb < threshold for >= mem_duration_ms_floor consecutive
    U006MemLowSustained,
    /// U-007: error_rate > threshold over window (min samples)
    U007ErrorRateExceeded,
    /// U-008: OP-HALT / EVT-...-HALT-EMIT verb observed on bus
    U008OpHaltObserved,
    /// U-009: >= law_threshold law violations in window
    U009LawViolationsExceeded,
    /// U-010: schema-drift detected (unknown schema version)
    U010SchemaDrift,
    /// U-011: peer transitions >= flap_threshold in window
    U011PeerFlapExceeded,
    /// U-012: bilateral quorum diverged on artifact
    U012QuorumSplit,
    /// U-013: cadence < floor sustained
    U013CadenceFloorBreached,
    /// U-014: state-file age > stale_ms
    U014StateFileStaleness,
}

/// Returns true if the verb is on the OP_HALT exact whitelist OR matches the narrow regex.
/// Per JS lines 17-18: avoid false-positive on any verb containing "-HALT".
pub fn is_op_halt_verb(verb: &str) -> bool {
    if verb == "OP-HALT" || verb == "EVT-HALT" {
        return true;
    }
    // Narrow rule: EVT-<UPPER>-HALT-EMIT (avoid generic "-HALT" containing verbs)
    verb.starts_with("EVT-") && verb.ends_with("-HALT-EMIT")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_can_only_go_to_probing() {
        assert_eq!(legal_transitions(PeerState::Dark), &[PeerState::Probing]);
    }

    #[test]
    fn alive_can_go_to_three_states() {
        let t = legal_transitions(PeerState::Alive);
        assert!(t.contains(&PeerState::Kicked));
        assert!(t.contains(&PeerState::Dark));
        assert!(t.contains(&PeerState::Probing));
    }

    #[test]
    fn illegal_transition_rejected() {
        let mut psm = PeerStateMachine::new("test-peer");
        // Dark → Alive is illegal (must go through Probing).
        let r = psm.transition(PeerState::Alive, "test", 100);
        assert!(r.is_err());
        assert_eq!(psm.state, PeerState::Dark);
    }

    #[test]
    fn legal_path_dark_probing_alive() {
        let mut psm = PeerStateMachine::new("test-peer");
        assert!(psm.transition(PeerState::Probing, "probe", 100).is_ok());
        assert!(psm.transition(PeerState::Alive, "got-reply", 200).is_ok());
        assert_eq!(psm.state, PeerState::Alive);
        assert_eq!(psm.history.len(), 2);
    }

    #[test]
    fn kick_overdue_after_deadline() {
        let mut psm = PeerStateMachine::new("test-peer");
        psm.transition(PeerState::Probing, "probe", 100).unwrap();
        psm.transition(PeerState::Alive, "alive", 200).unwrap();
        psm.on_kick("hi", 1000, 300).unwrap();
        assert!(!psm.is_overdue(1000));
        assert!(psm.is_overdue(2000));
    }

    #[test]
    fn op_halt_whitelist_matches() {
        assert!(is_op_halt_verb("OP-HALT"));
        assert!(is_op_halt_verb("EVT-HALT"));
        assert!(is_op_halt_verb("EVT-LIRIS-HALT-EMIT"));
        assert!(is_op_halt_verb("EVT-FALCON-HALT-EMIT"));
    }

    #[test]
    fn op_halt_does_not_match_loose_strings() {
        assert!(!is_op_halt_verb("HALT-EMIT")); // missing EVT- prefix
        assert!(!is_op_halt_verb("EVT-X-HALT")); // missing -EMIT suffix
        assert!(!is_op_halt_verb("EVT-SOMETHING-HALT-OTHER")); // wrong tail
        assert!(!is_op_halt_verb("ACER_OWNER_REFRESH_VERIFICATION")); // unrelated
    }

    #[test]
    fn slo_thresholds_match_canon_defaults() {
        let t = SloThresholds::default();
        assert_eq!(t.mem_free_mb_floor, 200);
        assert_eq!(t.err_rate_threshold_permille, 100);
        assert_eq!(t.flap_threshold, 5);
        assert_eq!(t.cadence_floor_ms, 5_000);
    }
}
