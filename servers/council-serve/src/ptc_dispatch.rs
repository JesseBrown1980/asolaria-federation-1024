//! ptc_dispatch — HyperHermes Programmatic Tool Calling, absorbed into the Host-8 council frame.
//!
//! Hermes's strongest operational lesson is collapsing an N-step tool chain into one inference turn:
//! the harness executes the chain, and only the final result returns to the agent context. In our
//! frame this module is a PURE PLANNER over a declarative allow-listed pipeline. It validates the
//! chain, estimates the saved turns/context bytes, and reports intent. It NEVER calls tools, opens
//! sockets, runs shell commands, mutates state, or fires the engine.

use std::collections::{HashMap, HashSet};

/// A single declared tool step in a programmatic tool-calling plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub op: ToolOp,
    pub from: Option<String>,
    pub bytes: usize,
}

/// The only tool operations this staged planner will accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOp {
    RecallSearch,
    FabricHealth,
    CanonIndex,
    McpHealth,
    LaneHealth,
    BranchFreshness,
    RecoveryPlan,
    Summarize,
    Select,
    RenderHbp,
}

impl ToolOp {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "recall_search" => Some(Self::RecallSearch),
            "fabric_health" => Some(Self::FabricHealth),
            "canon_index" => Some(Self::CanonIndex),
            "mcp_health" => Some(Self::McpHealth),
            "lane_health" => Some(Self::LaneHealth),
            "branch_freshness" => Some(Self::BranchFreshness),
            "recovery_plan" => Some(Self::RecoveryPlan),
            "summarize" => Some(Self::Summarize),
            "select" => Some(Self::Select),
            "render_hbp" => Some(Self::RenderHbp),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RecallSearch => "recall_search",
            Self::FabricHealth => "fabric_health",
            Self::CanonIndex => "canon_index",
            Self::McpHealth => "mcp_health",
            Self::LaneHealth => "lane_health",
            Self::BranchFreshness => "branch_freshness",
            Self::RecoveryPlan => "recovery_plan",
            Self::Summarize => "summarize",
            Self::Select => "select",
            Self::RenderHbp => "render_hbp",
        }
    }
}

/// Planner verdict for the whole declared chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStatus {
    Ready,
    Empty,
    Invalid(&'static str),
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Invalid(_) => "invalid",
        }
    }

    pub fn reason(&self) -> &'static str {
        match self {
            Self::Ready => "allowlisted_chain_valid",
            Self::Empty => "no_steps",
            Self::Invalid(r) => r,
        }
    }
}

/// Result of validating one pipeline. `execute` is always false in this staged module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub status: PlanStatus,
    pub steps: usize,
    pub raw_turns: usize,
    pub ptc_turns: usize,
    pub saved_turns: usize,
    pub total_bytes: usize,
    pub final_bytes: usize,
    pub saved_context_bytes: usize,
    pub final_step: String,
}

/// Validate a declarative PTC chain. The format is intentionally linear: a step may depend only on a
/// previously declared step. That rejects future references and cycles without graph search.
pub fn plan(steps: &[Step]) -> Plan {
    if steps.is_empty() {
        return Plan {
            status: PlanStatus::Empty,
            steps: 0,
            raw_turns: 0,
            ptc_turns: 0,
            saved_turns: 0,
            total_bytes: 0,
            final_bytes: 0,
            saved_context_bytes: 0,
            final_step: "-".to_string(),
        };
    }

    let mut seen = HashSet::new();
    for s in steps {
        if !valid_id(&s.id) {
            return invalid(steps, "invalid_step_id");
        }
        if !seen.insert(s.id.as_str()) {
            return invalid(steps, "duplicate_step_id");
        }
        if let Some(dep) = &s.from {
            if !seen.contains(dep.as_str()) {
                return invalid(steps, "missing_or_future_ref");
            }
        }
    }

    // A second defensive pass catches disconnected refs after duplicate validation and makes the
    // dependency map explicit for future extensions without changing today's linear rule.
    let ids: HashSet<_> = steps.iter().map(|s| s.id.as_str()).collect();
    let deps: HashMap<_, _> = steps
        .iter()
        .filter_map(|s| s.from.as_ref().map(|d| (s.id.as_str(), d.as_str())))
        .collect();
    if deps.values().any(|d| !ids.contains(d)) {
        return invalid(steps, "missing_ref");
    }

    let total_bytes = steps.iter().map(|s| s.bytes).sum::<usize>();
    let final_step = steps.last().unwrap();
    let final_bytes = final_step.bytes;
    let raw_turns = steps.len();
    let ptc_turns = 1;
    Plan {
        status: PlanStatus::Ready,
        steps: steps.len(),
        raw_turns,
        ptc_turns,
        saved_turns: raw_turns.saturating_sub(ptc_turns),
        total_bytes,
        final_bytes,
        saved_context_bytes: total_bytes.saturating_sub(final_bytes),
        final_step: final_step.id.clone(),
    }
}

fn invalid(steps: &[Step], reason: &'static str) -> Plan {
    let total_bytes = steps.iter().map(|s| s.bytes).sum::<usize>();
    Plan {
        status: PlanStatus::Invalid(reason),
        steps: steps.len(),
        raw_turns: steps.len(),
        ptc_turns: 0,
        saved_turns: 0,
        total_bytes,
        final_bytes: 0,
        saved_context_bytes: 0,
        final_step: "-".to_string(),
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, op: ToolOp, from: Option<&str>, bytes: usize) -> Step {
        Step {
            id: id.to_string(),
            op,
            from: from.map(str::to_string),
            bytes,
        }
    }

    #[test]
    fn ready_chain_collapses_turns_and_context() {
        let steps = vec![
            step("recall", ToolOp::RecallSearch, None, 4000),
            step("health", ToolOp::McpHealth, Some("recall"), 1000),
            step("final", ToolOp::RenderHbp, Some("health"), 200),
        ];
        let p = plan(&steps);
        assert_eq!(p.status, PlanStatus::Ready);
        assert_eq!(p.raw_turns, 3);
        assert_eq!(p.ptc_turns, 1);
        assert_eq!(p.saved_turns, 2);
        assert_eq!(p.saved_context_bytes, 5000);
        assert_eq!(p.final_step, "final");
    }

    #[test]
    fn empty_is_staged_not_fake_ready() {
        let p = plan(&[]);
        assert_eq!(p.status, PlanStatus::Empty);
        assert_eq!(p.ptc_turns, 0);
        assert_eq!(p.final_step, "-");
    }

    #[test]
    fn duplicate_ids_invalid() {
        let steps = vec![
            step("x", ToolOp::RecallSearch, None, 1),
            step("x", ToolOp::RenderHbp, Some("x"), 1),
        ];
        assert_eq!(
            plan(&steps).status,
            PlanStatus::Invalid("duplicate_step_id")
        );
    }

    #[test]
    fn future_ref_invalid_blocks_cycles() {
        let steps = vec![
            step("a", ToolOp::Summarize, Some("b"), 1),
            step("b", ToolOp::RenderHbp, Some("a"), 1),
        ];
        assert_eq!(
            plan(&steps).status,
            PlanStatus::Invalid("missing_or_future_ref")
        );
    }

    #[test]
    fn invalid_id_fails_closed() {
        let steps = vec![step("bad|id", ToolOp::RecallSearch, None, 1)];
        assert_eq!(plan(&steps).status, PlanStatus::Invalid("invalid_step_id"));
    }

    #[test]
    fn toolop_allowlist_parse() {
        assert_eq!(ToolOp::parse("fabric_health"), Some(ToolOp::FabricHealth));
        assert_eq!(ToolOp::parse("shell"), None);
    }
}
