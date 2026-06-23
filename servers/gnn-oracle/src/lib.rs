//! Phase-2.5 demote of kernel/core/src/gnn/mod.rs to userspace crate. cycle-70 operator AUTHORIZE_ALL.
//!
//! GNN inference lane · Phase-4 Steps 61-80
//!
//! Per REPO_LAW Invariant 3: GNN is THE inference primitive for routing, ranking, verdict-aggregation.
//! Pairs with liris's `LIRIS_GNN_BRIDGE_AUDIT` :82086 (6 invariants, 4 hard-zero + 2 bounded).
//!
//! v0.2 pixels-first CPU oracle: API + byte/edge measurement engine available before GPU.
//! GPU/ONNX remains a cold path behind explicit model-swap/cosign.

#![no_std]
#![forbid(unsafe_code)]
#![allow(unknown_lints)] // CI runs rustc 1.81; some allows below are 1.95-era lint names
#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(rustdoc::bare_urls)]
#![allow(rustdoc::broken_intra_doc_links)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Target edge count per `project_gnn_edges_canon_correction_2_16M_not_94K.md`.
pub const GNN_EDGES_TARGET: u32 = 2_158_671;

/// Inference latency budget p99 (REPO_LAW Invariant 7 + KERNEL_TARGETS.md Step 70).
pub const GNN_INFER_P99_LATENCY_MS_BUDGET: u32 = 100;

/// Batch processing target per Q-cohort canon (Step 71).
pub const GNN_BATCH_TARGET_PER_SEC: u32 = 40_783;

/// Current hot-path engine. Pixels/bytes before GPU; JSON is not required.
pub const GNN_ENGINE_MODE: &str = "pixels-first-cpu-v1";

/// Synthetic model fingerprint for the in-crate byte/edge engine.
pub const GNN_PIXELS_FIRST_SHA16: &str = "b8f1c0ffee8b17e5";

/// The old wave receipt's constant-fallback abort signature. A real frame score must not land here.
pub const GNN_ABORT_SCORE_Q: u32 = 120;

/// Quantized observed frame from the cadence boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedFrame {
    pub tick: u64,
    pub phash: u64,
    pub frame_delta: u32,
    pub entropy_q: u32,
    pub pid_fingerprint: u64,
}

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

/// GNN inference handle.
///
/// v0.2 is ready only after it binds a fresh observed frame from the cadence seam.
/// ONNX/GPU loading is deliberately still a cold path and remains unimplemented until
/// model-swap authorization exists.
pub struct GnnInference {
    latest_frame: Option<ObservedFrame>,
    last_scored_tick: u64,
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
            latest_frame: None,
            last_scored_tick: 0,
            model_sha16: Some(String::from(GNN_PIXELS_FIRST_SHA16)),
        }
    }

    /// `true` if a fresh, non-frozen observed frame is available for real measurement.
    pub fn is_ready(&self) -> bool {
        self.latest_frame
            .map(|frame| {
                frame.tick > self.last_scored_tick
                    && frame.frame_delta > 0
                    && frame.entropy_q > 0
                    && score_frame_q(frame) != GNN_ABORT_SCORE_Q
            })
            .unwrap_or(false)
    }

    /// Human-readable engine mode for HBP status surfaces.
    pub fn engine_mode(&self) -> &'static str {
        GNN_ENGINE_MODE
    }

    /// Fingerprint of the active hot-path engine.
    pub fn model_sha16(&self) -> Option<&str> {
        self.model_sha16.as_deref()
    }

    /// Ingest a quantized cadence frame. Older/replayed ticks are ignored.
    pub fn observe_frame(&mut self, frame: ObservedFrame) -> bool {
        if frame.tick <= self.latest_frame.map(|f| f.tick).unwrap_or(0) {
            return false;
        }
        self.latest_frame = Some(frame);
        true
    }

    /// Last observed frame, for HBP status surfaces.
    pub fn latest_frame(&self) -> Option<ObservedFrame> {
        self.latest_frame
    }

    /// Fixed-point preview of the latest observed frame score. Does not consume the tick.
    pub fn preview_latest_score_q(&self) -> Option<u32> {
        self.latest_frame.map(score_frame_q)
    }

    /// Score the latest frame and consume that tick so replay cannot be reused as fresh.
    pub fn score_latest_frame_q(&mut self) -> Result<u32, GnnErr> {
        let frame = self.latest_frame.ok_or(GnnErr::ModelNotLoaded)?;
        let score = score_frame_q(frame);
        if !self.is_ready() {
            return Err(GnnErr::InputShapeInvalid);
        }
        self.last_scored_tick = frame.tick;
        Ok(score)
    }

    /// Loads an ONNX model. v0.1 stub.
    pub fn load_onnx_model(
        &mut self,
        _model_bytes: &[u8],
        _expected_sha16: &str,
    ) -> Result<(), GnnErr> {
        Err(GnnErr::Unimplemented)
    }

    /// Pixels-first byte/edge score in [0, 1].
    ///
    /// This is not a hash repeat and not GPU inference: it measures byte transitions,
    /// printable density, entropy-ish spread, and edge alternation directly from the
    /// envelope bytes. It is stable, no_std, HBP-friendly, and available before GPU.
    pub fn score_bytes(&self, bytes: &[u8]) -> Result<f32, GnnErr> {
        Ok(score_bytes_pixels_first(bytes))
    }

    /// Routing oracle: predict best route for envelope using pixels-first byte score.
    pub fn predict_route(&self, envelope_bytes: &[u8]) -> Result<RoutingDecision, GnnErr> {
        let score = self.score_bytes(envelope_bytes)?;
        if score >= 0.86 {
            Ok(RoutingDecision::QuadBroadcast)
        } else if score >= 0.72 {
            Ok(RoutingDecision::TriadBroadcast)
        } else if score >= 0.58 {
            Ok(RoutingDecision::UnicastLiris)
        } else {
            Ok(RoutingDecision::Local)
        }
    }

    /// Top-N ranking. v0.2 ranks by byte/edge score, with deterministic fallback shape.
    pub fn rank_top_n(&self, items: &[String], n: usize) -> Result<Vec<RankedItem>, GnnErr> {
        let mut ranked: Vec<RankedItem> = items
            .iter()
            .map(|id| RankedItem {
                item_id: id.clone(),
                score: self.score_bytes(id.as_bytes()).unwrap_or(0.0),
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then_with(|| a.item_id.cmp(&b.item_id))
        });
        ranked.truncate(n);
        Ok(ranked)
    }

    /// Verdict aggregation. v0.2 gives bounded vote verdicts.
    pub fn aggregate_verdict(
        &self,
        votes_in_favor: u32,
        total_votes: u32,
    ) -> Result<AggregatedVerdict, GnnErr> {
        if total_votes == 0 {
            return Ok(AggregatedVerdict::Hold);
        }
        let pct = votes_in_favor as f32 / total_votes as f32;
        if pct >= 0.80 {
            Ok(AggregatedVerdict::ProceedStrong)
        } else if pct >= 0.60 {
            Ok(AggregatedVerdict::ProceedWeak)
        } else if pct > 0.40 {
            Ok(AggregatedVerdict::Hold)
        } else if pct >= 0.20 {
            Ok(AggregatedVerdict::Investigate)
        } else {
            Ok(AggregatedVerdict::Block)
        }
    }
}

/// Pixels-first fixed-point score from a real observed frame. Range 0..1000.
pub fn score_frame_q(frame: ObservedFrame) -> u32 {
    let delta_q = frame.frame_delta.min(1024) as u64;
    let entropy_q = frame.entropy_q.min(1024) as u64;
    let phash_bits = frame.phash.count_ones() as u64;
    let pid_bits = frame.pid_fingerprint.count_ones() as u64;
    let edge_mix = (frame.phash ^ frame.pid_fingerprint ^ frame.tick).count_ones() as u64;
    let raw = 40
        + delta_q * 250 / 1024
        + entropy_q * 260 / 1024
        + phash_bits * 180 / 64
        + pid_bits * 120 / 64
        + edge_mix * 150 / 64;
    raw.min(1000) as u32
}

/// Pixels-first byte/edge score in [0, 1] — pure, no_std, the reusable core of
/// `GnnInference::score_bytes`. Measures printable density, byte transitions, low-bit
/// alternation, nibble occupancy, and high-byte ratio directly from the bytes (not a hash
/// repeat, not GPU). Empty input scores 0.
pub fn score_bytes_pixels_first(bytes: &[u8]) -> f32 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut printable = 0u32;
    let mut controls = 0u32;
    let mut high = 0u32;
    let mut transitions = 0u32;
    let mut alternations = 0u32;
    let mut prev = bytes[0];
    let mut bins = [0u32; 16];
    for (i, b) in bytes.iter().copied().enumerate() {
        bins[(b & 0x0f) as usize] += 1;
        if b == b'\n' || b == b'\r' || b == b'\t' || (0x20..=0x7e).contains(&b) {
            printable += 1;
        } else if b < 0x20 || b == 0x7f {
            controls += 1;
        } else {
            high += 1;
        }
        if i > 0 {
            if b != prev {
                transitions += 1;
            }
            if (b ^ prev) & 1 == 1 {
                alternations += 1;
            }
            prev = b;
        }
    }
    let n = bytes.len() as f32;
    let printable_ratio = printable as f32 / n;
    let control_penalty = controls as f32 / n;
    let high_ratio = high as f32 / n;
    let transition_ratio = transitions as f32 / n.max(1.0);
    let alternation_ratio = alternations as f32 / n.max(1.0);
    let occupied = bins.iter().filter(|&&v| v > 0).count() as f32 / 16.0;
    let score = 0.28 * printable_ratio
        + 0.22 * transition_ratio
        + 0.18 * alternation_ratio
        + 0.18 * occupied
        + 0.10 * high_ratio
        + 0.04
        - 0.35 * control_penalty;
    score.clamp(0.0, 1.0)
}

// ===== STEP 3 — white-room Scorer + Omniflywheel verdict-aggregator =====
//
// The asolaria-whiteroom-engine SCORER leg, ported as a Rust trait: per mark, score in [0,1],
// KEEP genius (>= threshold) / COMPACT mistake (below — retained as cold provenance, NEVER
// deleted). The Omniflywheel (prof / verdict-aggregator, AgentRole::Omniflywheel) promotes a
// finding ONLY when its forward score clears the genius bar AND its reverse-gain risk is bounded
// — the loop's many-rooms -> reverse-gain GNN -> 1-answer fold. All PURE / E=0: scoring and
// classification only; no store, no I/O, no dispatch.

/// White-room genius threshold — KEEP at/above, COMPACT below (asolaria-whiteroom-engine canon).
pub const WHITEROOM_GENIUS_THRESHOLD: f32 = 0.72;

/// Omniflywheel maximum reverse-gain risk permitted for promotion.
pub const OMNIFLYWHEEL_MAX_REVERSE_RISK: f32 = 0.28;

/// Pluggable white-room scorer. Implementors map a mark's bytes to a score in [0, 1]. PURE:
/// a scorer has no store and performs no I/O (the white-room STORE is a separate concern).
pub trait Scorer {
    /// Score a mark's bytes in [0, 1].
    fn score(&self, mark: &[u8]) -> f32;
}

/// Pixels-first byte/edge scorer — the live hot-path scorer (no GPU, no hash repeat).
pub struct PixelsFirstScorer;
impl Scorer for PixelsFirstScorer {
    fn score(&self, mark: &[u8]) -> f32 {
        score_bytes_pixels_first(mark)
    }
}

/// Deterministic fallback scorer — stable, bounded, content-derived. For testable pipelines
/// at $0 (mirrors the node loop's deterministic-mock lane). NOT a quality signal — a placeholder
/// when the pixels-first/learned scorer is unavailable.
pub struct DeterministicScorer;
impl Scorer for DeterministicScorer {
    fn score(&self, mark: &[u8]) -> f32 {
        if mark.is_empty() {
            return 0.0;
        }
        let sum: u32 = mark.iter().map(|&b| b as u32).sum();
        (sum % 1000) as f32 / 1000.0
    }
}

/// White-room verdict for a scored mark. NEVER `Delete` — a mistake is COMPACTED (kept as cold
/// provenance), matching the white-room append-only / compact-not-delete doctrine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteRoomVerdict {
    /// score >= genius threshold — KEEP as genius (farm the gem).
    FarmGenius,
    /// below threshold — COMPACT as mistake (retained, never deleted).
    CompactMistake,
}

/// Classify a mark via a scorer into a white-room verdict. Returns `(score, verdict)`.
pub fn whiteroom_verdict<S: Scorer>(scorer: &S, mark: &[u8]) -> (f32, WhiteRoomVerdict) {
    let s = scorer.score(mark);
    let v = if s >= WHITEROOM_GENIUS_THRESHOLD {
        WhiteRoomVerdict::FarmGenius
    } else {
        WhiteRoomVerdict::CompactMistake
    };
    (s, v)
}

/// Omniflywheel promotion gate (prof / verdict-aggregator): promote a finding ONLY if its forward
/// score clears the genius threshold AND its reverse-gain risk is within bound. This is the
/// reverse-gain check the loop applies after the forward prism fold — a high forward score is
/// necessary but NOT sufficient; the reverse risk must also be low.
pub fn omniflywheel_promote(score: f32, reverse_risk: f32) -> bool {
    score >= WHITEROOM_GENIUS_THRESHOLD && reverse_risk <= OMNIFLYWHEEL_MAX_REVERSE_RISK
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
    fn new_inference_has_engine_but_waits_for_frame() {
        let g = GnnInference::new();
        assert!(!g.is_ready());
        assert_eq!(g.engine_mode(), GNN_ENGINE_MODE);
        assert_eq!(g.model_sha16(), Some(GNN_PIXELS_FIRST_SHA16));
    }

    #[test]
    fn observed_frame_makes_inference_ready_then_score_consumes_tick() {
        let mut g = GnnInference::new();
        let frame = ObservedFrame {
            tick: 1,
            phash: 0xf0f0_f0f0_f0f0_f0f0,
            frame_delta: 512,
            entropy_q: 500,
            pid_fingerprint: 0x0f0f_0f0f_0f0f_0f0f,
        };
        assert!(g.observe_frame(frame));
        assert!(g.is_ready());
        let score = g.score_latest_frame_q().unwrap();
        assert_ne!(score, GNN_ABORT_SCORE_Q);
        assert!(!g.is_ready());
    }

    #[test]
    fn frozen_or_zero_entropy_frame_is_not_ready() {
        let mut g = GnnInference::new();
        assert!(g.observe_frame(ObservedFrame {
            tick: 1,
            phash: 1,
            frame_delta: 0,
            entropy_q: 10,
            pid_fingerprint: 2,
        }));
        assert!(!g.is_ready());
        assert!(g.observe_frame(ObservedFrame {
            tick: 2,
            phash: 1,
            frame_delta: 10,
            entropy_q: 0,
            pid_fingerprint: 2,
        }));
        assert!(!g.is_ready());
    }

    #[test]
    fn route_uses_byte_measurement() {
        let g = GnnInference::new();
        let route = g
            .predict_route(b"HBPv1|row=edge|payload=abcd1234|json=0")
            .unwrap();
        assert!(matches!(
            route,
            RoutingDecision::Local
                | RoutingDecision::UnicastLiris
                | RoutingDecision::TriadBroadcast
                | RoutingDecision::QuadBroadcast
        ));
    }

    #[test]
    fn verdict_aggregates_votes() {
        let g = GnnInference::new();
        assert_eq!(
            g.aggregate_verdict(85, 100).unwrap(),
            AggregatedVerdict::ProceedStrong
        );
        assert_eq!(
            g.aggregate_verdict(50, 100).unwrap(),
            AggregatedVerdict::Hold
        );
        assert_eq!(
            g.aggregate_verdict(10, 100).unwrap(),
            AggregatedVerdict::Block
        );
    }

    #[test]
    fn rank_top_3_is_score_ordered() {
        let items = vec![
            String::from("a"),
            String::from("HBPv1|row=edge|payload=abcd1234|json=0"),
            String::from("zzzzzzzzzzzzzzzz"),
            String::from("d"),
        ];
        let ranked = GnnInference::new().rank_top_n(&items, 3).unwrap();
        assert_eq!(ranked.len(), 3);
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn control_bytes_are_penalized() {
        let g = GnnInference::new();
        let plain = g
            .score_bytes(b"HBPv1|row=edge|payload=abcd1234|json=0")
            .unwrap();
        let controls = g.score_bytes(&[0, 1, 2, 3, 4, 5, 6, 7]).unwrap();
        assert!(plain > controls);
    }

    // --- STEP 3 white-room scorer + Omniflywheel aggregator ---

    struct FixedScorer(f32);
    impl Scorer for FixedScorer {
        fn score(&self, _mark: &[u8]) -> f32 {
            self.0
        }
    }

    #[test]
    fn whiteroom_verdict_keeps_genius_compacts_mistake() {
        assert_eq!(
            whiteroom_verdict(&FixedScorer(0.80), b"x").1,
            WhiteRoomVerdict::FarmGenius
        );
        assert_eq!(
            whiteroom_verdict(&FixedScorer(0.50), b"x").1,
            WhiteRoomVerdict::CompactMistake
        );
        // exactly at the threshold counts as genius (>=).
        assert_eq!(
            whiteroom_verdict(&FixedScorer(WHITEROOM_GENIUS_THRESHOLD), b"x").1,
            WhiteRoomVerdict::FarmGenius
        );
        // the score is passed through unchanged.
        assert_eq!(whiteroom_verdict(&FixedScorer(0.80), b"x").0, 0.80);
    }

    #[test]
    fn omniflywheel_promote_needs_high_score_and_low_reverse_risk() {
        assert!(
            omniflywheel_promote(0.80, 0.10),
            "high score + low risk promotes"
        );
        assert!(
            !omniflywheel_promote(0.80, 0.40),
            "high reverse risk blocks promotion"
        );
        assert!(
            !omniflywheel_promote(0.60, 0.10),
            "low score blocks promotion"
        );
        // exactly at both bounds promotes (>= score, <= risk).
        assert!(omniflywheel_promote(
            WHITEROOM_GENIUS_THRESHOLD,
            OMNIFLYWHEEL_MAX_REVERSE_RISK
        ));
    }

    #[test]
    fn pixels_first_scorer_matches_gnn_score_bytes() {
        let s = PixelsFirstScorer;
        let g = GnnInference::new();
        let mark = b"HBPv1|row=edge|payload=abcd1234|json=0";
        assert_eq!(s.score(mark), g.score_bytes(mark).unwrap());
        assert_eq!(s.score(b""), 0.0);
    }

    #[test]
    fn deterministic_scorer_is_stable_and_bounded() {
        let s = DeterministicScorer;
        let a = s.score(b"hello world");
        assert_eq!(a, s.score(b"hello world"), "deterministic");
        assert!((0.0..=1.0).contains(&a), "bounded to [0,1]");
        assert_eq!(s.score(b""), 0.0);
    }
}
