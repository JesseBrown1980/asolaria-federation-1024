//! policy — claw-code 2.0's executable policy engine, absorbed into the Host-8 frame (Rust, json=0).
//! claw-code made merge/retry/rebase/stale-cleanup/escalation MACHINE-ENFORCED (PolicyRule:
//! condition -> action, priority). This session I triaged ~40 open PRs BY HAND (merge clean+CI-green+
//! additive / close superseded / rebase DIRTY / hold structural-or-author-gated). This encodes that
//! exact decision cascade as a deterministic, tested engine so the next backlog triages itself.
//! It DECIDES (computes the disposition + reason) — it NEVER merges, closes, rebases, or fires.
//! Execution stays the gated, operator step.

/// Signals about one open PR — the inputs the manual triage used.
#[derive(Debug, Clone)]
pub struct PrContext {
    pub id: String,
    /// CI all green OR no CI gate configured (some repos have none).
    pub ci_ok: bool,
    /// mergeStateStatus == CLEAN.
    pub clean: bool,
    /// mergeStateStatus == DIRTY / CONFLICTING.
    pub dirty: bool,
    pub draft: bool,
    /// A later PR/commit already did this work.
    pub superseded: bool,
    /// Docs / findings / receipts — low-risk additive.
    pub additive: bool,
    /// Private / exploit-tooling / structural — operator-only.
    pub sensitive: bool,
    /// PR body says do-not-merge-yet (awaiting attack-verify).
    pub author_forbids: bool,
}

/// The disposition — INTENT only; never executed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    Merge,
    Close,
    Rebase,
    Hold,
}

impl PolicyAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyAction::Merge => "merge",
            PolicyAction::Close => "close",
            PolicyAction::Rebase => "rebase",
            PolicyAction::Hold => "hold",
        }
    }
}

/// Evaluate the disposition for one PR via the safety-ordered cascade (highest-safety first):
/// sensitive / author-forbids -> HOLD; superseded -> CLOSE; dirty -> REBASE; draft -> HOLD;
/// clean + ci_ok + additive -> MERGE; everything else -> HOLD (low-confidence, the safe default).
/// Decides only — performs no merge/close/rebase/fire.
pub fn evaluate(c: &PrContext) -> (PolicyAction, &'static str) {
    if c.sensitive {
        return (
            PolicyAction::Hold,
            "sensitive: private/exploit/structural — operator decision",
        );
    }
    if c.author_forbids {
        return (
            PolicyAction::Hold,
            "author forbids merge until attack-verify",
        );
    }
    if c.superseded {
        return (PolicyAction::Close, "superseded by later work");
    }
    if c.dirty {
        return (
            PolicyAction::Rebase,
            "dirty/conflicting — rebase before any merge",
        );
    }
    if c.draft {
        return (PolicyAction::Hold, "draft — author must finalize");
    }
    if c.clean && c.ci_ok && c.additive {
        return (
            PolicyAction::Merge,
            "clean + ci-ok + additive + not-superseded",
        );
    }
    (PolicyAction::Hold, "low-confidence — needs review")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(id: &str) -> PrContext {
        PrContext {
            id: id.to_string(),
            ci_ok: true,
            clean: false,
            dirty: false,
            draft: false,
            superseded: false,
            additive: false,
            sensitive: false,
            author_forbids: false,
        }
    }

    #[test]
    fn clean_ci_additive_merges() {
        let mut c = base("p1");
        c.clean = true;
        c.additive = true;
        assert_eq!(evaluate(&c).0, PolicyAction::Merge);
    }

    #[test]
    fn superseded_closes() {
        let mut c = base("p2");
        c.superseded = true;
        c.clean = true;
        c.additive = true; // would merge, but superseded wins -> close
        assert_eq!(evaluate(&c).0, PolicyAction::Close);
    }

    #[test]
    fn dirty_rebases_even_if_additive() {
        let mut c = base("p3");
        c.dirty = true;
        c.additive = true;
        assert_eq!(evaluate(&c).0, PolicyAction::Rebase);
    }

    #[test]
    fn sensitive_holds_over_everything() {
        let mut c = base("p4");
        c.sensitive = true;
        c.clean = true;
        c.additive = true;
        c.superseded = true; // sensitive outranks close
        assert_eq!(evaluate(&c).0, PolicyAction::Hold);
    }

    #[test]
    fn author_forbids_holds() {
        let mut c = base("p5");
        c.author_forbids = true;
        c.clean = true;
        c.additive = true;
        assert_eq!(evaluate(&c).0, PolicyAction::Hold);
    }

    #[test]
    fn draft_holds() {
        let mut c = base("p6");
        c.draft = true;
        c.clean = true;
        c.additive = true;
        assert_eq!(evaluate(&c).0, PolicyAction::Hold);
    }

    #[test]
    fn clean_but_not_additive_holds_low_confidence() {
        let mut c = base("p7");
        c.clean = true; // not additive (e.g. code change) -> not auto-merge
        assert_eq!(evaluate(&c).0, PolicyAction::Hold);
    }

    #[test]
    fn clean_additive_but_ci_red_holds() {
        let mut c = base("p8");
        c.clean = true;
        c.additive = true;
        c.ci_ok = false; // CI red -> never auto-merge
        assert_eq!(evaluate(&c).0, PolicyAction::Hold);
    }

    #[test]
    fn empty_signals_default_to_hold_not_merge() {
        // a bare PR with no signals must NOT auto-merge (safe default)
        assert_eq!(evaluate(&base("p9")).0, PolicyAction::Hold);
    }
}
