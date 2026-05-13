//! GNN inference lane · Phase-4 Steps 61-80
//!
//! Per REPO_LAW Invariant 3: GNN is THE inference primitive for routing, ranking, verdict-aggregation.
//! Pairs with liris's `LIRIS_GNN_BRIDGE_AUDIT` :82086 (6 invariants, 4 hard-zero + 2 bounded).
//!
//! v0.1 scaffold: API + ONNX-loaded-model placeholder + deterministic fallback.
//! Phase-4 wave wires real `ggml` no_std inference + edge accounting (2.16M edges from canon).

use alloc::string::String;
use alloc::vec::Vec;

/// Target edge count per `project_gnn_edges_canon_correction_2_16M_not_94K.md`.
pub const GNN_EDGES_TARGET: u32 = 2_158_671;

/// Inference latency budget p99 (REPO_LAW Invariant 7 + KERNEL_TARGETS.md Step 70).
pub const GNN_INFER_P99_LATENCY_MS_BUDGET: u32 = 100;

/// Batch processing target per Q-cohort canon (Step 71).
pub const GNN_BATCH_TARGET_PER_SEC: u32 = 40_783;

/// GNN inference errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GnnErr {
    /// Model not yet loaded.
    ModelNotLoaded,
    /// Input shape mismatch with model expectation.
    InputShapeInvalid,
    /// Inference exceeded p99 latency budget.
    LatencyBudgetExceeded,
    /// Cosign rejection of model swap.
    ModelSwapUnauthorized,
    /// Stub not yet implemented.
    Unimplemented,
}

/// Routing oracle decision: where should this envelope go?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Keep local.
    Local,
    /// Broadcast to TRIAD (Falcon-Acer-Liris).
    TriadBroadcast,
    /// Broadcast to QUAD (+ Aether).
    QuadBroadcast,
    /// Direct unicast to a specific vantage.
    UnicastAcer,
    UnicastLiris,
    UnicastFalcon,
    UnicastAether,
}

/// Top-N ranked result from GNN.
#[derive(Debug, Clone)]
pub struct RankedItem {
    pub item_id: String,
    pub score: f32,
}

/// Verdict-aggregator output from GNN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregatedVerdict {
    /// Strong PROCEED (≥80% supervisor votes in favor).
    ProceedStrong,
    /// Weak PROCEED (60-80%).
    ProceedWeak,
    /// HOLD (40-60%, no clear majority).
    Hold,
    /// BLOCK (≤40%).
    Block,
    /// INVESTIGATE (matches LIRIS_BUS_BIAS_RECEIPTS edge case).
    Investigate,
}

/// Deterministic fallback for `top_n` when GNN unavailable.
/// Returns the first `n` items in input order (no ranking). Used in Phase-4 v0.1.
pub fn top_n_deterministic_fallback(items: &[String], n: usize) -> Vec<RankedItem> {
    items
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, id)| RankedItem {
            item_id: id.clone(),
            score: 1.0 - (i as f32) * 0.001,
        })
        .collect()
}

/// GNN inference handle. v0.1: model not loaded; calls return Unimplemented.
pub struct GnnInference {
    model_loaded: bool,
    model_sha16: Option<String>,
}

impl Default for GnnInference {
    fn default() -> Self {
        Self::new()
    }
}

impl GnnInference {
    pub fn new() -> Self {
        Self {
            model_loaded: false,
            model_sha16: None,
        }
    }

    /// `true` if a model has been loaded for inference.
    pub fn is_ready(&self) -> bool {
        self.model_loaded
    }

    /// Loads an ONNX model. v0.1 stub.
    pub fn load_onnx_model(
        &mut self,
        _model_bytes: &[u8],
        _expected_sha16: &str,
    ) -> Result<(), GnnErr> {
        Err(GnnErr::Unimplemented)
    }

    /// Routing oracle: predict best route for envelope. v0.1 fallback returns Local.
    pub fn predict_route(&self, _envelope_bytes: &[u8]) -> Result<RoutingDecision, GnnErr> {
        if !self.model_loaded {
            return Ok(RoutingDecision::Local);
        }
        Err(GnnErr::Unimplemented)
    }

    /// Top-N ranking. v0.1 returns deterministic fallback (in-order, no ranking).
    pub fn rank_top_n(&self, items: &[String], n: usize) -> Result<Vec<RankedItem>, GnnErr> {
        if !self.model_loaded {
            return Ok(top_n_deterministic_fallback(items, n));
        }
        Err(GnnErr::Unimplemented)
    }

    /// Verdict aggregation. v0.1 returns Hold (conservative default).
    pub fn aggregate_verdict(
        &self,
        _votes_in_favor: u32,
        _total_votes: u32,
    ) -> Result<AggregatedVerdict, GnnErr> {
        if !self.model_loaded {
            return Ok(AggregatedVerdict::Hold);
        }
        Err(GnnErr::Unimplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    #[test]
    fn gnn_edges_target_is_2_16M() {
        assert_eq!(GNN_EDGES_TARGET, 2_158_671);
    }

    #[test]
    fn latency_budget_is_100ms() {
        assert_eq!(GNN_INFER_P99_LATENCY_MS_BUDGET, 100);
    }

    #[test]
    fn batch_target_matches_q_cohort_canon() {
        assert_eq!(GNN_BATCH_TARGET_PER_SEC, 40_783);
    }

    #[test]
    fn new_inference_not_ready() {
        let g = GnnInference::new();
        assert!(!g.is_ready());
    }

    #[test]
    fn fallback_route_is_local() {
        let g = GnnInference::new();
        assert_eq!(g.predict_route(b"test").unwrap(), RoutingDecision::Local);
    }

    #[test]
    fn fallback_verdict_is_hold() {
        let g = GnnInference::new();
        assert_eq!(
            g.aggregate_verdict(50, 100).unwrap(),
            AggregatedVerdict::Hold
        );
    }

    #[test]
    fn deterministic_fallback_top_3() {
        let items = vec![
            String::from("a"),
            String::from("b"),
            String::from("c"),
            String::from("d"),
        ];
        let ranked = top_n_deterministic_fallback(&items, 3);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].item_id, "a");
        assert!(ranked[0].score > ranked[1].score);
    }
}
