use asolaria_kernel_core::syscall::HookwallVerdict;
use asolaria_server_fischer_eval::{
    evaluate, FischerEnvelope, FischerEval, FischerScore, FischerVerdict,
};

pub(crate) struct SpawnCognition {
    pub(crate) eval: FischerEval,
    pub(crate) spawn_gate_verdict: HookwallVerdict,
    pub(crate) final_verdict: HookwallVerdict,
}

impl SpawnCognition {
    pub(crate) fn flags_csv(&self) -> String {
        if self.eval.flags.is_empty() {
            "clean".to_string()
        } else {
            self.eval.flags.join(",")
        }
    }
}

pub(crate) fn evaluate_launch_plan(
    instance_pid: &str,
    tuple_verb: &str,
    noun: &str,
    room_folder: &str,
    runner_kind: &str,
    forward_score_q: u32,
    reverse_risk_q: u32,
    spawn_gate_verdict: HookwallVerdict,
) -> SpawnCognition {
    let envelope = FischerEnvelope {
        pid: Some(instance_pid.to_string()),
        actor: "host8-serve".to_string(),
        verb: "spawn".to_string(),
        target: format!("{}:{}", runner_kind, noun),
        payload: format!(
            "tuple_verb={};noun={};room_folder={}",
            tuple_verb, noun, room_folder
        ),
        payload_json_true: false,
        hbp_path: Some("launch-plan.hbp".to_string()),
        sidecar_plan: Some("host8-launch-plan".to_string()),
        ledger_path: Some("host8-launch-plan.hbp".to_string()),
        tuple: Some(format!("{}|{}", tuple_verb, noun)),
        cube_47d: Some(room_folder.to_string()),
        cosign: None,
        halt_path: Some("/halt".to_string()),
        authority_jump: false,
        recursive_consent: false,
        operator_witness_required: false,
        operator_witness: false,
    };
    let score = FischerScore {
        composite: format!("forward_q={};reverse_q={}", forward_score_q, reverse_risk_q),
        l0_real: forward_score_q > 0,
        shannon: f64::from(forward_score_q.min(1000)) / 1000.0,
        g4_state: "HOST8_LAUNCH_PLAN".to_string(),
    };
    let eval = evaluate(&envelope, &score, true);
    let final_verdict = strictest_verdict(spawn_gate_verdict, eval.verdict);
    SpawnCognition {
        eval,
        spawn_gate_verdict,
        final_verdict,
    }
}

pub(crate) fn strictest_verdict(
    spawn_gate_verdict: HookwallVerdict,
    fischer_verdict: FischerVerdict,
) -> HookwallVerdict {
    match fischer_verdict {
        FischerVerdict::Block | FischerVerdict::Refute => HookwallVerdict::Block,
        FischerVerdict::Hold | FischerVerdict::Analyze => match spawn_gate_verdict {
            HookwallVerdict::Block => HookwallVerdict::Block,
            _ => HookwallVerdict::Hold,
        },
        FischerVerdict::Proceed => spawn_gate_verdict,
    }
}

pub(crate) fn fischer_verdict_str(verdict: FischerVerdict) -> &'static str {
    match verdict {
        FischerVerdict::Proceed => "PROCEED",
        FischerVerdict::Hold => "HOLD",
        FischerVerdict::Block => "BLOCK",
        FischerVerdict::Refute => "REFUTE",
        FischerVerdict::Analyze => "ANALYZE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fischer_proceed_cannot_loosen_spawn_hold() {
        assert_eq!(
            strictest_verdict(HookwallVerdict::Hold, FischerVerdict::Proceed),
            HookwallVerdict::Hold
        );
    }

    #[test]
    fn fischer_hold_tightens_spawn_proceed() {
        assert_eq!(
            strictest_verdict(HookwallVerdict::Proceed, FischerVerdict::Hold),
            HookwallVerdict::Hold
        );
    }

    #[test]
    fn fischer_refute_blocks_even_when_spawn_proceeds() {
        assert_eq!(
            strictest_verdict(HookwallVerdict::Proceed, FischerVerdict::Refute),
            HookwallVerdict::Block
        );
    }

    #[test]
    fn default_launch_plan_cognition_preserves_process_hold() {
        let cognition = evaluate_launch_plan(
            "0155964ffc8ef1f8",
            "summon",
            "AGT-TEST",
            "omni-room-behcs-256-1",
            "opencode",
            0,
            0,
            HookwallVerdict::Hold,
        );
        assert_eq!(cognition.spawn_gate_verdict, HookwallVerdict::Hold);
        assert_eq!(cognition.final_verdict, HookwallVerdict::Hold);
        assert_eq!(cognition.eval.verdict, FischerVerdict::Proceed);
    }
}
