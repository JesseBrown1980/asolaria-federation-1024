use asolaria_kernel_core::syscall::HookwallVerdict;
use asolaria_server_fischer_eval::{
    evaluate, FischerEnvelope, FischerEval, FischerScore, FischerVerdict,
};

pub(crate) struct SpawnCognition {
    pub(crate) eval: FischerEval,
    pub(crate) spawn_gate_verdict: HookwallVerdict,
    pub(crate) final_verdict: HookwallVerdict,
}

pub(crate) struct LaunchPlanCognitionInput<'a> {
    pub(crate) instance_pid: &'a str,
    pub(crate) tuple_verb: &'a str,
    pub(crate) noun: &'a str,
    pub(crate) room_folder: &'a str,
    pub(crate) runner_kind: &'a str,
    pub(crate) forward_score_q: u32,
    pub(crate) reverse_risk_q: u32,
    pub(crate) spawn_gate_verdict: HookwallVerdict,
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

pub(crate) fn evaluate_launch_plan(input: LaunchPlanCognitionInput<'_>) -> SpawnCognition {
    let envelope = FischerEnvelope {
        pid: Some(input.instance_pid.to_string()),
        actor: "host8-serve".to_string(),
        verb: "spawn".to_string(),
        target: format!("{}:{}", input.runner_kind, input.noun),
        payload: format!(
            "tuple_verb={};noun={};room_folder={}",
            input.tuple_verb, input.noun, input.room_folder
        ),
        payload_json_true: false,
        hbp_path: Some("launch-plan.hbp".to_string()),
        sidecar_plan: Some("host8-launch-plan".to_string()),
        ledger_path: Some("host8-launch-plan.hbp".to_string()),
        tuple: Some(format!("{}|{}", input.tuple_verb, input.noun)),
        cube_47d: Some(input.room_folder.to_string()),
        cosign: None,
        halt_path: Some("/halt".to_string()),
        authority_jump: false,
        recursive_consent: false,
        operator_witness_required: false,
        operator_witness: false,
    };
    let score = FischerScore {
        composite: format!(
            "forward_q={};reverse_q={}",
            input.forward_score_q, input.reverse_risk_q
        ),
        l0_real: input.forward_score_q > 0,
        shannon: f64::from(input.forward_score_q.min(1000)) / 1000.0,
        g4_state: "HOST8_LAUNCH_PLAN".to_string(),
    };
    let eval = evaluate(&envelope, &score, true);
    let final_verdict = strictest_verdict(input.spawn_gate_verdict, eval.verdict);
    SpawnCognition {
        eval,
        spawn_gate_verdict: input.spawn_gate_verdict,
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
        let cognition = evaluate_launch_plan(LaunchPlanCognitionInput {
            instance_pid: "0155964ffc8ef1f8",
            tuple_verb: "summon",
            noun: "AGT-TEST",
            room_folder: "omni-room-behcs-256-1",
            runner_kind: "opencode",
            forward_score_q: 0,
            reverse_risk_q: 0,
            spawn_gate_verdict: HookwallVerdict::Hold,
        });
        assert_eq!(cognition.spawn_gate_verdict, HookwallVerdict::Hold);
        assert_eq!(cognition.final_verdict, HookwallVerdict::Hold);
        assert_eq!(cognition.eval.verdict, FischerVerdict::Proceed);
    }
}
