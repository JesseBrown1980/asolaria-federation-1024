//! stale_branch — claw-code 2.0's stale-branch detection, absorbed into the Host-8 frame (Rust, json=0).
//! claw-code added BranchFreshness (Fresh/Stale/Diverged) + a policy (AutoRebase/AutoMergeForward/
//! WarnOnly/Block) so a RED test on a stale side-branch is classified as stale-noise, not a new
//! regression — and so the right refresh action (fast-forward vs rebase vs block) is chosen
//! automatically. This is the freshness half that pairs with policy.rs (which decides "rebase"; this
//! decides whether that rebase is a clean fast-forward, a real rebase, or a must-block conflict).
//! Pure + read-only: it ASSESSES + RECOMMENDS — it never rebases, merges, or fires.

/// How fresh a branch is relative to its base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Required branch comparison fields are missing/invalid. Fail closed.
    Unknown,
    /// Up to date with base (behind_by == 0). The branch's own commits (ahead_by) are fine.
    Fresh,
    /// Behind base with NO own commits (behind_by > 0, ahead_by == 0) — a clean fast-forward.
    Stale,
    /// Behind base AND has own commits (behind_by > 0 && ahead_by > 0) — needs a rebase/merge.
    Diverged,
}

impl Freshness {
    pub fn as_str(&self) -> &'static str {
        match self {
            Freshness::Unknown => "unknown",
            Freshness::Fresh => "fresh",
            Freshness::Stale => "stale",
            Freshness::Diverged => "diverged",
        }
    }
}

/// The recommended refresh action (intent only — never executed here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshAction {
    Ready,        // fresh — nothing to do
    MergeForward, // stale, clean — fast-forward to base
    Rebase,       // diverged, clean — replay own commits onto base
    Block,        // conflicting — manual resolve required
}

impl RefreshAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            RefreshAction::Ready => "ready",
            RefreshAction::MergeForward => "merge_forward",
            RefreshAction::Rebase => "rebase",
            RefreshAction::Block => "block",
        }
    }
}

/// Assess freshness from the ahead/behind counts (as `gh api .../compare` reports them).
pub fn assess(ahead_by: Option<u64>, behind_by: Option<u64>) -> Freshness {
    let (Some(ahead_by), Some(behind_by)) = (ahead_by, behind_by) else {
        return Freshness::Unknown;
    };
    if behind_by == 0 {
        Freshness::Fresh
    } else if ahead_by == 0 {
        Freshness::Stale
    } else {
        Freshness::Diverged
    }
}

/// Recommend the refresh action. `conflicting` = git/GitHub reports a real conflict (mergeStateStatus
/// DIRTY). Decides intent only.
pub fn recommend(freshness: Freshness, conflicting: Option<bool>) -> (RefreshAction, &'static str) {
    if freshness == Freshness::Unknown {
        return (
            RefreshAction::Block,
            "missing ahead/behind counts — cannot assess freshness",
        );
    }
    if freshness == Freshness::Fresh {
        return (RefreshAction::Ready, "up to date with base");
    }
    let Some(conflicting) = conflicting else {
        return (
            RefreshAction::Block,
            "missing conflict status — cannot choose refresh action",
        );
    };
    if conflicting {
        return (
            RefreshAction::Block,
            "conflicting — manual rebase/resolve required",
        );
    }
    match freshness {
        Freshness::Stale => (
            RefreshAction::MergeForward,
            "behind, no own commits — fast-forward to base",
        ),
        Freshness::Diverged => (
            RefreshAction::Rebase,
            "behind + own commits — rebase onto base",
        ),
        Freshness::Unknown => (
            RefreshAction::Block,
            "missing ahead/behind counts — cannot assess freshness",
        ),
        Freshness::Fresh => (RefreshAction::Ready, "up to date with base"),
    }
}

/// claw-code's key insight: a RED result on a non-fresh branch is likely STALE NOISE (the branch is
/// missing base fixes), not a new regression — re-evaluate after refresh before treating it as real.
pub fn is_stale_noise(freshness: Freshness) -> bool {
    freshness != Freshness::Fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assess_maps_ahead_behind_to_freshness() {
        assert_eq!(assess(Some(3), Some(0)), Freshness::Fresh); // ahead only = up to date with base
        assert_eq!(assess(Some(0), Some(0)), Freshness::Fresh);
        assert_eq!(assess(Some(0), Some(5)), Freshness::Stale); // behind, no own commits
        assert_eq!(assess(Some(2), Some(5)), Freshness::Diverged); // behind + own commits
        assert_eq!(assess(None, Some(0)), Freshness::Unknown);
        assert_eq!(assess(Some(0), None), Freshness::Unknown);
    }

    #[test]
    fn recommend_fresh_is_ready() {
        assert_eq!(
            recommend(Freshness::Fresh, Some(false)).0,
            RefreshAction::Ready
        );
        // even if "conflicting" is mis-set, fresh short-circuits to ready
        assert_eq!(
            recommend(Freshness::Fresh, Some(true)).0,
            RefreshAction::Ready
        );
        // and fresh does not require conflict status.
        assert_eq!(recommend(Freshness::Fresh, None).0, RefreshAction::Ready);
    }

    #[test]
    fn recommend_stale_clean_is_merge_forward() {
        assert_eq!(
            recommend(Freshness::Stale, Some(false)).0,
            RefreshAction::MergeForward
        );
    }

    #[test]
    fn recommend_diverged_clean_is_rebase() {
        assert_eq!(
            recommend(Freshness::Diverged, Some(false)).0,
            RefreshAction::Rebase
        );
    }

    #[test]
    fn recommend_conflicting_blocks() {
        assert_eq!(
            recommend(Freshness::Diverged, Some(true)).0,
            RefreshAction::Block
        );
        assert_eq!(
            recommend(Freshness::Stale, Some(true)).0,
            RefreshAction::Block
        );
    }

    #[test]
    fn recommend_missing_data_blocks() {
        assert_eq!(
            recommend(Freshness::Unknown, Some(false)).0,
            RefreshAction::Block
        );
        assert_eq!(recommend(Freshness::Stale, None).0, RefreshAction::Block);
        assert_eq!(recommend(Freshness::Diverged, None).0, RefreshAction::Block);
    }

    #[test]
    fn stale_noise_flags_non_fresh_only() {
        assert!(!is_stale_noise(Freshness::Fresh));
        assert!(is_stale_noise(Freshness::Unknown));
        assert!(is_stale_noise(Freshness::Stale));
        assert!(is_stale_noise(Freshness::Diverged));
    }
}
