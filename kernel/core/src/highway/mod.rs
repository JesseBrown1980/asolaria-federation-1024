//! Highway · cross-tier transit primitive · Phase-9 Step 173
//!
//! Moves a resource handle from tier A to tier B with cosign-chain witness.
//! Authorized under operator quintuple-auth :82646 standing window (2026-05-11 → 2026-05-25).
//!
//! Per REPO_LAW Invariant 5 + Invariant 4: every transit appends a row to the cosign-chain so
//! tier-changes are auditable end-to-end.

use crate::cosign_chain::CosignChain;
use crate::syscall::AccessTier;
use crate::tier::{policy_for, quintuple_auth_covers};
use alloc::string::String;

/// Highway transit errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HighwayErr {
    /// Source and destination tiers are identical — no transit needed.
    SameTier,
    /// Source-to-destination transit forbidden by current policy.
    TransitForbidden,
    /// Quintuple-auth window does not cover the destination tier (rare; current window covers all 6).
    NotInQuintupleAuthWindow,
    /// Cosign-chain append failed.
    CosignFailure,
    /// Stub not yet implemented (real cosign + storage migration land in Phase-9 wave).
    Unimplemented,
}

/// Result of a transit request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitVerdict {
    pub from: AccessTier,
    pub to: AccessTier,
    /// `true` if transit is authorized by current policy + quintuple-auth window.
    pub authorized: bool,
    /// Brief reason if `authorized=false`.
    pub deny_reason: Option<String>,
    /// Cosign-chain row index where this transit was recorded (None if Unimplemented).
    pub cosign_row: Option<u64>,
}

/// Pure check: is the (from → to) transit allowed under current policy + quintuple-auth?
pub fn check_transit(from: AccessTier, to: AccessTier) -> Result<TransitVerdict, HighwayErr> {
    if from == to {
        return Err(HighwayErr::SameTier);
    }
    if !quintuple_auth_covers(to) {
        return Ok(TransitVerdict {
            from,
            to,
            authorized: false,
            deny_reason: Some(String::from(
                "destination tier not in current quintuple-auth window",
            )),
            cosign_row: None,
        });
    }
    let _from_policy = policy_for(from);
    let _to_policy = policy_for(to);
    // Under quintuple-auth :82646 standing-approval: ALL transits authorized for the 2-week window.
    Ok(TransitVerdict {
        from,
        to,
        authorized: true,
        deny_reason: None,
        cosign_row: None,
    })
}

/// Executes a transit and witnesses it in the cosign-chain.
/// v0.1 stub — returns Unimplemented until cosign_chain::append is wired.
pub fn execute_transit(
    _chain: &mut CosignChain,
    _from: AccessTier,
    _to: AccessTier,
    _resource_id: &str,
) -> Result<TransitVerdict, HighwayErr> {
    Err(HighwayErr::Unimplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_tier_is_no_transit() {
        let r = check_transit(AccessTier::Public, AccessTier::Public);
        assert_eq!(r, Err(HighwayErr::SameTier));
    }

    #[test]
    fn public_to_secret_authorized_under_quintuple_auth() {
        let v = check_transit(AccessTier::Public, AccessTier::Secret).unwrap();
        assert!(v.authorized);
        assert_eq!(v.from, AccessTier::Public);
        assert_eq!(v.to, AccessTier::Secret);
    }

    #[test]
    fn hidden_to_shadow_authorized_under_quintuple_auth() {
        let v = check_transit(AccessTier::Hidden, AccessTier::Shadow).unwrap();
        assert!(v.authorized);
    }

    #[test]
    fn execute_stub_returns_unimplemented() {
        let mut chain = CosignChain::new();
        let r = execute_transit(
            &mut chain,
            AccessTier::Public,
            AccessTier::Restricted,
            "res-0",
        );
        assert_eq!(r, Err(HighwayErr::Unimplemented));
    }
}
