//! recovery — claw-code 2.0's recovery recipes + bounded-retry ledger, absorbed into the Host-8 frame
//! (Rust, json=0). claw-code pairs each worker-startup failure with a RECIPE (the action that would
//! fix it) and a bounded retry budget so a wedged lane is retried a few times and then ESCALATED
//! rather than retried forever. This is the capstone of the lane absorbs: it consumes lane_health's
//! `StartupFailure` classification and decides the recovery action.
//!
//! CLAIMS-GATE: it DECIDES the recovery action (intent) — it NEVER restarts, reconnects, re-sends, or
//! fires. And it fails CLOSED: a lane that has exhausted its retry budget ESCALATES to the operator
//! (never silently "recovering" forever — the recovery analogue of the bounded-work DoS discipline),
//! and an `Unknown` failure escalates immediately (blind retry can't fix an unclassified cause).

use crate::lane_health::StartupFailure;

/// The recommended recovery action (intent only — never executed here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    ReissueTrust,       // trust_required
    ResendPrompt,       // prompt_misdelivery / prompt_acceptance_timeout
    ReconnectTransport, // transport_dead
    RestartWorker,      // worker_crashed
    Escalate,           // unknown cause OR retry budget exhausted -> operator
}

impl RecoveryAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecoveryAction::ReissueTrust => "reissue_trust",
            RecoveryAction::ResendPrompt => "resend_prompt",
            RecoveryAction::ReconnectTransport => "reconnect_transport",
            RecoveryAction::RestartWorker => "restart_worker",
            RecoveryAction::Escalate => "escalate",
        }
    }
    /// True when the action is to escalate (no automatic recovery — needs the operator).
    pub fn is_escalation(&self) -> bool {
        matches!(self, RecoveryAction::Escalate)
    }
}

/// The recovery RECIPE for a failure mode, ignoring retry budget: the action that (would) address it.
/// `Unknown` has no automatic recipe -> Escalate.
pub fn recipe(failure: StartupFailure) -> (RecoveryAction, &'static str) {
    match failure {
        StartupFailure::TrustRequired => (
            RecoveryAction::ReissueTrust,
            "re-issue the trust prompt/nonce",
        ),
        StartupFailure::PromptMisdelivery => (
            RecoveryAction::ResendPrompt,
            "prompt misdelivered — re-send to the lane",
        ),
        StartupFailure::PromptAcceptanceTimeout => (
            RecoveryAction::ResendPrompt,
            "prompt not accepted in time — re-send",
        ),
        StartupFailure::TransportDead => (
            RecoveryAction::ReconnectTransport,
            "transport dead — reconnect, then re-send",
        ),
        StartupFailure::WorkerCrashed => (
            RecoveryAction::RestartWorker,
            "worker crashed — restart the process",
        ),
        StartupFailure::Unknown => (
            RecoveryAction::Escalate,
            "cause unclear — escalate to operator",
        ),
    }
}

/// Recommend the recovery action given how many attempts have ALREADY been made for this lane. Fails
/// closed: at/over budget -> Escalate (never retry forever). Decides intent only — performs nothing.
pub fn recommend(
    failure: StartupFailure,
    attempts: u32,
    max_attempts: u32,
) -> (RecoveryAction, &'static str) {
    if attempts >= max_attempts {
        return (
            RecoveryAction::Escalate,
            "retry budget exhausted — escalate to operator",
        );
    }
    recipe(failure)
}

/// Parse a failure mode from its canonical `StartupFailure::as_str()` form. Unrecognized -> None.
pub fn parse_failure(s: &str) -> Option<StartupFailure> {
    Some(match s {
        "trust_required" => StartupFailure::TrustRequired,
        "prompt_misdelivery" => StartupFailure::PromptMisdelivery,
        "prompt_acceptance_timeout" => StartupFailure::PromptAcceptanceTimeout,
        "transport_dead" => StartupFailure::TransportDead,
        "worker_crashed" => StartupFailure::WorkerCrashed,
        "unknown" => StartupFailure::Unknown,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_maps_each_failure() {
        assert_eq!(
            recipe(StartupFailure::TrustRequired).0,
            RecoveryAction::ReissueTrust
        );
        assert_eq!(
            recipe(StartupFailure::PromptMisdelivery).0,
            RecoveryAction::ResendPrompt
        );
        assert_eq!(
            recipe(StartupFailure::PromptAcceptanceTimeout).0,
            RecoveryAction::ResendPrompt
        );
        assert_eq!(
            recipe(StartupFailure::TransportDead).0,
            RecoveryAction::ReconnectTransport
        );
        assert_eq!(
            recipe(StartupFailure::WorkerCrashed).0,
            RecoveryAction::RestartWorker
        );
        assert_eq!(recipe(StartupFailure::Unknown).0, RecoveryAction::Escalate);
    }

    #[test]
    fn recommend_within_budget_uses_recipe() {
        let (a, _) = recommend(StartupFailure::WorkerCrashed, 1, 3);
        assert_eq!(a, RecoveryAction::RestartWorker);
        assert!(!a.is_escalation());
    }

    #[test]
    fn recommend_at_or_over_budget_escalates() {
        // CLAIMS-GATE: a lane that exhausted its retries escalates, never retries forever.
        assert_eq!(
            recommend(StartupFailure::WorkerCrashed, 3, 3).0,
            RecoveryAction::Escalate
        );
        assert_eq!(
            recommend(StartupFailure::TransportDead, 5, 3).0,
            RecoveryAction::Escalate
        );
    }

    #[test]
    fn unknown_escalates_even_with_budget() {
        let (a, _) = recommend(StartupFailure::Unknown, 0, 3);
        assert_eq!(a, RecoveryAction::Escalate); // blind retry can't fix an unclassified cause
    }

    #[test]
    fn zero_budget_escalates_immediately() {
        assert_eq!(
            recommend(StartupFailure::WorkerCrashed, 0, 0).0,
            RecoveryAction::Escalate
        );
    }

    #[test]
    fn parse_failure_round_trips_as_str() {
        for f in [
            StartupFailure::TrustRequired,
            StartupFailure::PromptMisdelivery,
            StartupFailure::PromptAcceptanceTimeout,
            StartupFailure::TransportDead,
            StartupFailure::WorkerCrashed,
            StartupFailure::Unknown,
        ] {
            assert_eq!(parse_failure(f.as_str()), Some(f));
        }
        assert_eq!(parse_failure("not_a_failure"), None);
    }
}
