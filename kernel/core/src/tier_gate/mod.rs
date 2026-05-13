//! Tier-gating enforcement primitive · Phase-9 Module 1
//!
//! Per Phase-9-Prep scout + cycle-72 dashboard canon: enforce the **7-tier** access
//! matrix at the caller→target decision layer. This module is pure policy —
//! `no_std`-safe, no allocation, no I/O.
//!
//! Cycle-72: SOVEREIGNTY added as 7th tier per liris dashboard Drive Registry —
//! "operator-physical-only / NEVER agent-driven" (USB-SOVLINUX-2TB · D-DRIVE-LIRIS).
//! No agent caller (regardless of cosign) can reach Sovereignty targets; only the
//! Sovereignty tier itself (operator-physical) can access anything.
//!
//! Wiring into `syscall` is performed by the Stoker scout in a non-overlapping change.
//!
//! Decision matrix (caller → target):
//! - **Public**     caller: Public+Restricted → Allow; Stealth..Sovereignty → Deny
//! - **Restricted** caller: Public..Stealth → Allow; Hidden/Shadow/Secret → Escalate; **Sovereignty → Deny**
//! - **Stealth**    caller: same-or-lower → Allow; Hidden/Shadow/Secret → Escalate; **Sovereignty → Deny**
//! - **Hidden**     caller: same-or-lower → Allow; Shadow/Secret → Escalate; **Sovereignty → Deny**
//! - **Shadow**     caller: lower-or-equal → Allow; Secret/Sovereignty → Deny (no escalation path)
//! - **Secret**     caller: lower-or-equal → Allow; **Sovereignty → Deny**
//! - **Sovereignty** caller (operator-physical): Allow everything
//!
//! Tier ordering (ascending sensitivity):
//! Public < Restricted < Stealth < Hidden < Shadow < Secret < Sovereignty.
//! This matches `AccessTier` enum declaration order in `crate::syscall`.

use crate::syscall::AccessTier;

/// Outcome of a caller→target tier gate decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierGateVerdict {
    /// Caller may access the target tier directly, no further authority required.
    Allow,
    /// Caller is structurally denied access; no escalation path exists.
    Deny,
    /// Caller may attempt access via cosign/operator escalation (out-of-band authority).
    Escalate,
}

/// Map an `AccessTier` to its ordinal rank (ascending sensitivity).
///
/// Public=0, Restricted=1, Stealth=2, Hidden=3, Shadow=4, Secret=5, Sovereignty=6.
#[inline]
const fn tier_rank(tier: AccessTier) -> u8 {
    match tier {
        AccessTier::Public => 0,
        AccessTier::Restricted => 1,
        AccessTier::Stealth => 2,
        AccessTier::Hidden => 3,
        AccessTier::Shadow => 4,
        AccessTier::Secret => 5,
        AccessTier::Sovereignty => 6,
    }
}

/// Returns the gate verdict for a `(caller, target)` tier pair.
///
/// See module-level docs for the full decision matrix.
///
/// **Sovereignty rule (cycle-72):** Sovereignty as a TARGET always Denies for any
/// non-Sovereignty caller — no escalation path exists, since Sovereignty is
/// operator-physical-only (no quintuple-cosign override). Sovereignty as a CALLER
/// always Allows (operator-physical authority is universal).
pub fn check(caller_tier: AccessTier, target_tier: AccessTier) -> TierGateVerdict {
    let caller = tier_rank(caller_tier);
    let target = tier_rank(target_tier);

    // Universal rule: Sovereignty target is Deny for any non-Sovereignty caller.
    if target_tier == AccessTier::Sovereignty && caller_tier != AccessTier::Sovereignty {
        return TierGateVerdict::Deny;
    }

    match caller_tier {
        // Sovereignty caller: Allow everything (operator-physical universal authority).
        AccessTier::Sovereignty => TierGateVerdict::Allow,
        // Public: Allow Public+Restricted, Deny everything else (no escalation path).
        AccessTier::Public => {
            if target <= tier_rank(AccessTier::Restricted) {
                TierGateVerdict::Allow
            } else {
                TierGateVerdict::Deny
            }
        }
        // Restricted: Allow Public+Restricted+Stealth, Escalate Hidden/Shadow/Secret.
        AccessTier::Restricted => {
            if target <= tier_rank(AccessTier::Stealth) {
                TierGateVerdict::Allow
            } else {
                TierGateVerdict::Escalate
            }
        }
        // Stealth + Hidden: Allow same-or-lower, Escalate higher.
        AccessTier::Stealth | AccessTier::Hidden => {
            if target <= caller {
                TierGateVerdict::Allow
            } else {
                TierGateVerdict::Escalate
            }
        }
        // Shadow + Secret: Allow lower-or-equal, Deny higher (no escalation path).
        AccessTier::Shadow | AccessTier::Secret => {
            if target <= caller {
                TierGateVerdict::Allow
            } else {
                TierGateVerdict::Deny
            }
        }
    }
}

/// Boolean convenience view of [`check`]. Returns `true` iff the verdict is `Allow`.
///
/// Escalate and Deny both yield `false` — callers needing to distinguish them
/// must use [`check`] directly.
pub fn tier_can_access(caller_tier: AccessTier, target_tier: AccessTier) -> bool {
    matches!(check(caller_tier, target_tier), TierGateVerdict::Allow)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: all 7 tiers in ascending-sensitivity order (cycle-72).
    const ALL_TIERS: [AccessTier; 7] = [
        AccessTier::Public,
        AccessTier::Restricted,
        AccessTier::Stealth,
        AccessTier::Hidden,
        AccessTier::Shadow,
        AccessTier::Secret,
        AccessTier::Sovereignty,
    ];

    #[test]
    fn public_caller_allows_public_and_restricted_denies_rest() {
        assert_eq!(
            check(AccessTier::Public, AccessTier::Public),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Public, AccessTier::Restricted),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Public, AccessTier::Stealth),
            TierGateVerdict::Deny
        );
        assert_eq!(
            check(AccessTier::Public, AccessTier::Hidden),
            TierGateVerdict::Deny
        );
        assert_eq!(
            check(AccessTier::Public, AccessTier::Shadow),
            TierGateVerdict::Deny
        );
        assert_eq!(
            check(AccessTier::Public, AccessTier::Secret),
            TierGateVerdict::Deny
        );
        // tier_can_access mirror:
        assert!(tier_can_access(AccessTier::Public, AccessTier::Restricted));
        assert!(!tier_can_access(AccessTier::Public, AccessTier::Hidden));
    }

    #[test]
    fn restricted_caller_allows_through_stealth_escalates_higher() {
        assert_eq!(
            check(AccessTier::Restricted, AccessTier::Public),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Restricted, AccessTier::Restricted),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Restricted, AccessTier::Stealth),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Restricted, AccessTier::Hidden),
            TierGateVerdict::Escalate
        );
        assert_eq!(
            check(AccessTier::Restricted, AccessTier::Shadow),
            TierGateVerdict::Escalate
        );
        assert_eq!(
            check(AccessTier::Restricted, AccessTier::Secret),
            TierGateVerdict::Escalate
        );
    }

    #[test]
    fn stealth_and_hidden_callers_allow_same_or_lower_escalate_higher() {
        // Stealth
        assert_eq!(
            check(AccessTier::Stealth, AccessTier::Public),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Stealth, AccessTier::Stealth),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Stealth, AccessTier::Hidden),
            TierGateVerdict::Escalate
        );
        assert_eq!(
            check(AccessTier::Stealth, AccessTier::Secret),
            TierGateVerdict::Escalate
        );
        // Hidden
        assert_eq!(
            check(AccessTier::Hidden, AccessTier::Restricted),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Hidden, AccessTier::Hidden),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Hidden, AccessTier::Shadow),
            TierGateVerdict::Escalate
        );
        assert_eq!(
            check(AccessTier::Hidden, AccessTier::Secret),
            TierGateVerdict::Escalate
        );
    }

    #[test]
    fn shadow_and_secret_callers_allow_lower_or_equal_deny_higher_no_escalation() {
        // Shadow: Allow up to Shadow, Deny Secret + Sovereignty (no escalation path).
        assert_eq!(
            check(AccessTier::Shadow, AccessTier::Public),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Shadow, AccessTier::Hidden),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Shadow, AccessTier::Shadow),
            TierGateVerdict::Allow
        );
        assert_eq!(
            check(AccessTier::Shadow, AccessTier::Secret),
            TierGateVerdict::Deny
        );
        assert_eq!(
            check(AccessTier::Shadow, AccessTier::Sovereignty),
            TierGateVerdict::Deny
        );
        // Secret: Allow all lower-or-equal; Deny Sovereignty (cycle-72 — was Allow all when only 6 tiers).
        for &t in &ALL_TIERS {
            if t == AccessTier::Sovereignty {
                assert_eq!(
                    check(AccessTier::Secret, t),
                    TierGateVerdict::Deny,
                    "Secret caller should Deny Sovereignty (no escalation)"
                );
            } else {
                assert_eq!(
                    check(AccessTier::Secret, t),
                    TierGateVerdict::Allow,
                    "Secret caller should Allow target {:?}",
                    t
                );
            }
        }
        // Shadow and Secret must never emit Escalate (no escalation path canonically).
        for &t in &ALL_TIERS {
            assert_ne!(check(AccessTier::Shadow, t), TierGateVerdict::Escalate);
            assert_ne!(check(AccessTier::Secret, t), TierGateVerdict::Escalate);
        }
    }

    #[test]
    fn sovereignty_target_always_denies_for_non_sovereignty_caller() {
        // Cycle-72 canon: Sovereignty is operator-physical-only / NEVER agent-driven.
        // No quintuple-cosign override exists. Even Secret-tier caller is Denied.
        for &c in &ALL_TIERS {
            if c == AccessTier::Sovereignty {
                continue;
            }
            assert_eq!(
                check(c, AccessTier::Sovereignty),
                TierGateVerdict::Deny,
                "{:?} caller must be Denied Sovereignty target (no escalation)",
                c
            );
        }
    }

    #[test]
    fn sovereignty_caller_allows_all_targets_operator_physical_universal() {
        // Sovereignty as a CALLER represents operator-physical authority (USB-SOVLINUX-2TB
        // touch, D-DRIVE-LIRIS direct manipulation). It can reach any tier.
        for &t in &ALL_TIERS {
            assert_eq!(
                check(AccessTier::Sovereignty, t),
                TierGateVerdict::Allow,
                "Sovereignty caller should Allow target {:?}",
                t
            );
        }
    }

    #[test]
    fn matrix_totals_match_canon_allow_deny_escalate_counts_7_tiers() {
        // Sweep the full 7×7 = 49 caller×target matrix and count verdicts.
        // Canon expectation post-cycle-72 (Sovereignty added):
        //   Public      : 2 Allow,  5 Deny, 0 Escalate
        //   Restricted  : 3 Allow,  1 Deny, 3 Escalate
        //   Stealth     : 3 Allow,  1 Deny, 3 Escalate
        //   Hidden      : 4 Allow,  1 Deny, 2 Escalate
        //   Shadow      : 5 Allow,  2 Deny, 0 Escalate
        //   Secret      : 6 Allow,  1 Deny, 0 Escalate
        //   Sovereignty : 7 Allow,  0 Deny, 0 Escalate
        // Totals: 30 Allow, 11 Deny, 8 Escalate (sum = 49 = 7×7).
        let mut allow = 0u32;
        let mut deny = 0u32;
        let mut escalate = 0u32;
        for &c in &ALL_TIERS {
            for &t in &ALL_TIERS {
                match check(c, t) {
                    TierGateVerdict::Allow => allow += 1,
                    TierGateVerdict::Deny => deny += 1,
                    TierGateVerdict::Escalate => escalate += 1,
                }
            }
        }
        assert_eq!(allow, 30, "Allow count drift");
        assert_eq!(deny, 11, "Deny count drift");
        assert_eq!(escalate, 8, "Escalate count drift");
        assert_eq!(allow + deny + escalate, 49);

        // tier_can_access must be true exactly the Allow set.
        let mut bool_allow = 0u32;
        for &c in &ALL_TIERS {
            for &t in &ALL_TIERS {
                if tier_can_access(c, t) {
                    bool_allow += 1;
                }
            }
        }
        assert_eq!(bool_allow, allow);
    }
}
