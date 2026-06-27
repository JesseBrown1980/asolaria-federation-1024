//! lane_health — claw-code 2.0's worker-boot lifecycle + startup failure classifier, absorbed into the
//! Host-8 frame (Rust, json=0). The tool that built Asolaria made "ready but idle" vs "prompt sent but
//! not accepted" a first-class distinction plus a 6-way startup failure classifier, so a stalled worker
//! reports WHY instead of looking generically "stuck". We hit exactly that this session (agent-wave
//! starvation / "why is it stuck"). This is the same idea our way: a pure, read-only diagnosis over lane
//! evidence — it classifies, it NEVER spawns or fires. Also folds in the lane-event dedupe that fixes
//! "phantom completions" (count UNIQUE terminal lanes, not re-logged terminal events).

use std::collections::HashSet;

/// Worker/lane lifecycle states (claw-code worker_boot states).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    Spawning,
    TrustRequired,
    ReadyForPrompt,
    PromptAccepted,
    Running,
    Blocked,
    Finished,
    Failed,
}

impl WorkerState {
    pub fn parse(s: &str) -> WorkerState {
        match s {
            "spawning" => WorkerState::Spawning,
            "trust_required" => WorkerState::TrustRequired,
            "ready_for_prompt" => WorkerState::ReadyForPrompt,
            "prompt_accepted" => WorkerState::PromptAccepted,
            "running" => WorkerState::Running,
            "blocked" => WorkerState::Blocked,
            "finished" => WorkerState::Finished,
            "failed" => WorkerState::Failed,
            _ => WorkerState::Spawning,
        }
    }
}

/// Why a stalled worker isn't progressing — claw-code's 6 variants, no vague "stuck".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupFailure {
    TrustRequired,
    PromptMisdelivery,
    PromptAcceptanceTimeout,
    TransportDead,
    WorkerCrashed,
    Unknown,
}

impl StartupFailure {
    pub fn as_str(&self) -> &'static str {
        match self {
            StartupFailure::TrustRequired => "trust_required",
            StartupFailure::PromptMisdelivery => "prompt_misdelivery",
            StartupFailure::PromptAcceptanceTimeout => "prompt_acceptance_timeout",
            StartupFailure::TransportDead => "transport_dead",
            StartupFailure::WorkerCrashed => "worker_crashed",
            StartupFailure::Unknown => "unknown",
        }
    }
}

/// Evidence about a lane/worker at the moment we ask "why isn't it progressing?".
#[derive(Debug, Clone)]
pub struct LaneEvidence {
    pub id: String,
    pub state: WorkerState,
    pub transport_ok: bool,
    pub trust_prompt_detected: bool,
    pub prompt_sent: bool,
    pub prompt_accepted: bool,
    pub misdelivered: bool,
    pub crashed: bool,
    pub elapsed_ms: u64,
    pub terminal: bool,
}

/// Diagnosis for one lane: either progressing/ok (None) or a named stall reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneDiagnosis {
    pub id: String,
    pub state: WorkerState,
    pub failure: Option<StartupFailure>,
    /// True when the lane is simply "ready but idle" — NOT a failure; the orchestrator just hasn't
    /// sent the prompt yet. This is the distinction that stops false "stuck" alarms.
    pub ready_but_idle: bool,
}

/// Classify WHY a worker isn't progressing. Returns the named failure, or None when it is genuinely
/// progressing / still within its acceptance window / merely ready-but-idle (see LaneDiagnosis).
/// Pure + read-only: it diagnoses, it never spawns or fires.
pub fn classify_stall(e: &LaneEvidence, acceptance_timeout_ms: u64) -> Option<StartupFailure> {
    if matches!(e.state, WorkerState::Running | WorkerState::Finished) {
        return None; // progressing
    }
    if e.crashed || e.state == WorkerState::Failed {
        return Some(StartupFailure::WorkerCrashed);
    }
    if !e.transport_ok {
        return Some(StartupFailure::TransportDead);
    }
    if e.trust_prompt_detected || e.state == WorkerState::TrustRequired {
        return Some(StartupFailure::TrustRequired);
    }
    if e.prompt_sent && !e.prompt_accepted {
        if e.misdelivered {
            return Some(StartupFailure::PromptMisdelivery);
        }
        if e.elapsed_ms >= acceptance_timeout_ms {
            return Some(StartupFailure::PromptAcceptanceTimeout);
        }
        return None; // sent, within the acceptance window — pending, not yet a failure
    }
    if e.state == WorkerState::ReadyForPrompt && !e.prompt_sent {
        return None; // ready but idle — orchestrator's move, NOT a stall (the key claw-code insight)
    }
    if e.elapsed_ms >= acceptance_timeout_ms {
        return Some(StartupFailure::Unknown); // genuinely stalled, cause unclear (down-ranked bucket)
    }
    None // still early
}

/// Full diagnosis (failure + the ready-but-idle flag).
pub fn diagnose(e: &LaneEvidence, acceptance_timeout_ms: u64) -> LaneDiagnosis {
    let failure = classify_stall(e, acceptance_timeout_ms);
    let ready_but_idle =
        failure.is_none() && e.state == WorkerState::ReadyForPrompt && !e.prompt_sent;
    LaneDiagnosis {
        id: e.id.clone(),
        state: e.state,
        failure,
        ready_but_idle,
    }
}

/// Count UNIQUE terminal lanes (dedupe re-logged terminal events) — fixes "phantom completions":
/// a resumed run re-emits terminal events, so counting raw terminal lines over-counts done-lanes.
pub fn count_unique_terminal(evidence: &[LaneEvidence]) -> usize {
    let mut seen = HashSet::new();
    for e in evidence {
        if e.terminal {
            seen.insert(e.id.as_str());
        }
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str, state: WorkerState) -> LaneEvidence {
        LaneEvidence {
            id: id.to_string(),
            state,
            transport_ok: true,
            trust_prompt_detected: false,
            prompt_sent: false,
            prompt_accepted: false,
            misdelivered: false,
            crashed: false,
            elapsed_ms: 0,
            terminal: false,
        }
    }

    #[test]
    fn ready_but_idle_is_not_a_failure() {
        let mut e = ev("a", WorkerState::ReadyForPrompt);
        e.elapsed_ms = 999_999;
        assert_eq!(classify_stall(&e, 5_000), None);
        assert!(
            diagnose(&e, 5_000).ready_but_idle,
            "ready+no-prompt = idle, not stuck"
        );
    }

    #[test]
    fn prompt_sent_not_accepted_within_window_is_pending() {
        let mut e = ev("b", WorkerState::ReadyForPrompt);
        e.prompt_sent = true;
        e.elapsed_ms = 1_000;
        assert_eq!(classify_stall(&e, 5_000), None);
    }

    #[test]
    fn prompt_sent_not_accepted_past_sla_is_timeout() {
        let mut e = ev("c", WorkerState::ReadyForPrompt);
        e.prompt_sent = true;
        e.elapsed_ms = 6_000;
        assert_eq!(
            classify_stall(&e, 5_000),
            Some(StartupFailure::PromptAcceptanceTimeout)
        );
    }

    #[test]
    fn misdelivery_is_distinct_from_timeout() {
        let mut e = ev("d", WorkerState::ReadyForPrompt);
        e.prompt_sent = true;
        e.misdelivered = true;
        e.elapsed_ms = 100;
        assert_eq!(
            classify_stall(&e, 5_000),
            Some(StartupFailure::PromptMisdelivery)
        );
    }

    #[test]
    fn transport_dead_and_trust_and_crash_each_named() {
        let mut t = ev("e", WorkerState::Spawning);
        t.transport_ok = false;
        assert_eq!(
            classify_stall(&t, 5_000),
            Some(StartupFailure::TransportDead)
        );
        let mut tr = ev("f", WorkerState::TrustRequired);
        assert_eq!(
            classify_stall(&tr, 5_000),
            Some(StartupFailure::TrustRequired)
        );
        tr.trust_prompt_detected = true;
        assert_eq!(
            classify_stall(&tr, 5_000),
            Some(StartupFailure::TrustRequired)
        );
        let mut cr = ev("g", WorkerState::Spawning);
        cr.crashed = true;
        assert_eq!(
            classify_stall(&cr, 5_000),
            Some(StartupFailure::WorkerCrashed)
        );
    }

    #[test]
    fn running_and_finished_are_not_stalls() {
        let mut r = ev("h", WorkerState::Running);
        r.elapsed_ms = 999_999;
        assert_eq!(classify_stall(&r, 5_000), None);
        assert_eq!(classify_stall(&ev("i", WorkerState::Finished), 5_000), None);
    }

    #[test]
    fn unique_terminal_dedupes_relogged_completions() {
        let mut a = ev("lane1", WorkerState::Finished);
        a.terminal = true;
        let mut a2 = ev("lane1", WorkerState::Finished); // re-logged on resume
        a2.terminal = true;
        let mut b = ev("lane2", WorkerState::Finished);
        b.terminal = true;
        // 3 terminal events, but only 2 UNIQUE done-lanes (the phantom-completion fix)
        assert_eq!(count_unique_terminal(&[a, a2, b]), 2);
    }
}
