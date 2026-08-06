//! confidence-scheduled verify — the DSpark lesson applied to the crank.
//!
//! DeepSeek's DSpark (Confidence-Scheduled Speculative Decoding) fixes "verification waste": a fast
//! drafter proposes many tokens, but indiscriminately running the EXPENSIVE verifier on all of them
//! burns scarce capacity on high-rejection-risk proposals. The fix: rank by expected acceptance and
//! spend the verify budget only where it returns.
//!
//! Asolaria already does a crude draft-and-verify: the cube / recalled-prior PROPOSES (the draft),
//! the GNN / reverse-gain / white-room pass VERIFIES (accept/reject), and the loop crank fires the
//! survivors. But the crank pages proposals by veto-window maturation alone (flat). This module is
//! the principled version: a cheap expected-acceptance prior ranks pending proposals, and only the
//! highest-survival ones are selected for the expensive verify; the rest DEFER (held, not dropped).
//!
//! HARD INVARIANT: this is a PLANNING function. It computes verify *intent* only — it NEVER fires.
//! `Scheduled::fire` is always false here; actual firing stays the gated, operator-emit step.

/// A pending proposal to be (maybe) verified. Fields are cheap signals already produced upstream:
/// `score` = GNN ranking confidence, `genius`/`risk` = reverse-gain rates. All in [0,1].
#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: String,
    /// All three are q in 0..=1000 (per-mille; integer only) — same unit as `gnn_score_q`.
    pub score_q: u16,
    pub genius_q: u16,
    pub risk_q: u16,
}

/// One scheduled decision. `verify` = selected for the expensive verify pass; `fire` is ALWAYS false.
#[derive(Debug, Clone, PartialEq)]
pub struct Scheduled {
    pub id: String,
    /// Expected-acceptance prior as q in 0..=1000 (per-mille; integer only).
    pub confidence_q: u16,
    pub rank: usize,
    pub verify: bool,
    pub fire: bool,
}

/// Cheap expected-acceptance ("survival") prior, clamped to [0,1]. Cheap BY DESIGN — the whole point
/// is to ration the EXPENSIVE verify with a cheap signal. score + genius lift acceptance, risk cuts it.
pub fn expected_acceptance_q(p: &Proposal) -> u16 {
    // (score + genius) / 2 - risk, clamped to 0..=1000. Integer only; the halving is a single
    // integer divide of the summed pair, so no per-term rounding.
    let lift = (p.score_q as u32 + p.genius_q as u32) / 2;
    let v = lift.saturating_sub(p.risk_q as u32);
    if v > 1000 { 1000 } else { v as u16 }
}

/// Rank proposals by expected acceptance (desc, stable tiebreak by id); select the top `verify_budget`
/// that also clear `accept_threshold` for the expensive verify pass; the rest DEFER (held). Never fires.
pub fn confidence_schedule(
    proposals: &[Proposal],
    verify_budget: usize,
    accept_threshold_q: u16,
) -> Vec<Scheduled> {
    let mut order: Vec<usize> = (0..proposals.len()).collect();
    order.sort_by(|&a, &b| {
        // integer ordering — total, no partial_cmp/NaN case
        expected_acceptance_q(&proposals[b])
            .cmp(&expected_acceptance_q(&proposals[a]))
            .then_with(|| proposals[a].id.cmp(&proposals[b].id))
    });
    let mut verified = 0usize;
    order
        .iter()
        .enumerate()
        .map(|(rank, &i)| {
            let confidence_q = expected_acceptance_q(&proposals[i]);
            let verify = verified < verify_budget && confidence_q >= accept_threshold_q;
            if verify {
                verified += 1;
            }
            Scheduled {
                id: proposals[i].id.clone(),
                confidence_q,
                rank,
                verify,
                fire: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, score_q: u16, genius_q: u16, risk_q: u16) -> Proposal {
        Proposal {
            id: id.to_string(),
            score,
            genius,
            risk,
        }
    }

    #[test]
    fn expected_acceptance_is_clamped_and_risk_penalized() {
        assert_eq!(expected_acceptance_q(&p("a", 1000, 1000, 0)), 1000);
        assert_eq!(expected_acceptance_q(&p("b", 0, 0, 1000)), 0); // clamped, not negative
        let lo = expected_acceptance_q(&p("c", 800, 800, 600));
        let hi = expected_acceptance_q(&p("d", 800, 800, 0));
        assert!(hi > lo, "risk must lower expected acceptance");
    }

    #[test]
    fn ranks_high_survival_first() {
        let props = [p("low", 0.1, 0.1, 0.0), p("high", 0.9, 0.9, 0.0)];
        let s = confidence_schedule(&props, 2, 0);
        assert_eq!(s[0].id, "high");
        assert_eq!(s[0].rank, 0);
        assert_eq!(s[1].id, "low");
    }

    #[test]
    fn verify_budget_is_respected() {
        let props = [
            p("a", 0.9, 0.9, 0.0),
            p("b", 0.8, 0.8, 0.0),
            p("c", 0.7, 0.7, 0.0),
        ];
        let s = confidence_schedule(&props, 2, 0);
        assert_eq!(
            s.iter().filter(|x| x.verify).count(),
            2,
            "budget=2 -> 2 verified"
        );
        // the two highest-confidence get verify; the lowest defers
        assert!(!s.iter().find(|x| x.id == "c").unwrap().verify);
    }

    #[test]
    fn threshold_prevents_wasted_verify_on_low_survival() {
        // budget is generous but everything is below threshold -> nothing verified (no waste)
        let props = [p("a", 0.2, 0.2, 0.0), p("b", 0.1, 0.1, 0.0)];
        let s = confidence_schedule(&props, 10, 500);
        assert_eq!(
            s.iter().filter(|x| x.verify).count(),
            0,
            "all below threshold -> 0 verified"
        );
    }

    #[test]
    fn schedule_never_fires() {
        let props = [p("a", 1.0, 1.0, 0.0), p("b", 0.9, 0.9, 0.0)];
        let s = confidence_schedule(&props, 10, 0);
        assert!(
            s.iter().all(|x| !x.fire),
            "scheduling must never fire (engine stays gated)"
        );
    }

    #[test]
    fn empty_is_empty_not_faked() {
        let s = confidence_schedule(&[], 8, 0.5);
        assert!(s.is_empty());
    }

    #[test]
    fn dspark_property_high_score_low_risk_beats_low_score_high_risk() {
        let props = [p("weak", 0.3, 0.3, 0.4), p("strong", 0.85, 0.85, 0.05)];
        let s = confidence_schedule(&props, 1, 500);
        let strong = s.iter().find(|x| x.id == "strong").unwrap();
        let weak = s.iter().find(|x| x.id == "weak").unwrap();
        assert!(
            strong.verify,
            "the high-survival proposal gets the scarce verify slot"
        );
        assert!(
            !weak.verify,
            "the low-survival proposal is deferred, not verified"
        );
    }
}
