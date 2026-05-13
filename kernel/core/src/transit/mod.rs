//! Transit executor · Phase-9 Module 2 · Forge
//!
//! Formalizes the cross-tier transit executor. Per Phase-9-Prep scout, the highway module
//! already exposes `check_transit` / `execute_transit` stubs; this module wraps the gate-check
//! into a concrete executor that:
//!
//! 1. Validates the tier-gate (delegates to `crate::highway::check_transit` — the canonical
//!    tier-gate primitive backed by `crate::tier::policy_for` + `quintuple_auth_covers`).
//! 2. Validates envelope size ≤ 64 KiB (Phase-9 transit-envelope budget).
//! 3. Returns a synthetic monotonically-increasing transit-id `u64`.
//!
//! NOTE: This module does NOT perform actual envelope transport — that responsibility
//! belongs to the `envelope` module. This is purely the executor authorization shell.
//!
//! Authorized under operator quintuple-auth standing window (2026-05-11 → 2026-05-25).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::highway::{check_transit, HighwayErr};
use crate::syscall::AccessTier;

/// Maximum envelope payload accepted by a single transit (64 KiB).
pub const MAX_TRANSIT_ENVELOPE_BYTES: usize = 64 * 1024;

/// A transit request: move an envelope from `from_tier` to `to_tier` carrying `envelope_size` bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitRequest {
    /// Source access tier.
    pub from_tier: AccessTier,
    /// Destination access tier.
    pub to_tier: AccessTier,
    /// Envelope payload size in bytes. Must be ≤ `MAX_TRANSIT_ENVELOPE_BYTES`.
    pub envelope_size: usize,
}

/// Transit executor errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransitErr {
    /// Source and destination tiers are identical — no transit needed.
    TierMismatch,
    /// Envelope payload exceeds `MAX_TRANSIT_ENVELOPE_BYTES`.
    OversizedPayload,
    /// Current quintuple-auth window does not cover the destination tier.
    QuintupleAuthRequired,
    /// Cosign-chain append failed (reserved for future wire-up).
    ChainAppendFailed,
}

/// Monotonically-increasing transit-id allocator. Synthetic; not persisted.
static NEXT_TRANSIT_ID: AtomicU64 = AtomicU64::new(1);

/// Executes a transit request.
///
/// Returns a synthetic transit-id on success. Does NOT actually move bytes — envelope
/// transport is the envelope module's responsibility. This function only:
///
/// - calls the tier-gate (`highway::check_transit`)
/// - enforces the 64 KiB envelope-size budget
/// - allocates a monotonic transit-id
pub fn execute(req: TransitRequest) -> Result<u64, TransitErr> {
    if req.envelope_size > MAX_TRANSIT_ENVELOPE_BYTES {
        return Err(TransitErr::OversizedPayload);
    }

    // Tier-gate: highway::check_transit is the canonical tier-gate primitive.
    match check_transit(req.from_tier, req.to_tier) {
        Err(HighwayErr::SameTier) => return Err(TransitErr::TierMismatch),
        Err(_) => return Err(TransitErr::ChainAppendFailed),
        Ok(verdict) => {
            if !verdict.authorized {
                return Err(TransitErr::QuintupleAuthRequired);
            }
        }
    }

    let id = NEXT_TRANSIT_ID.fetch_add(1, Ordering::SeqCst);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_mismatch_is_denied() {
        let req = TransitRequest {
            from_tier: AccessTier::Public,
            to_tier: AccessTier::Public,
            envelope_size: 128,
        };
        assert_eq!(execute(req), Err(TransitErr::TierMismatch));
    }

    #[test]
    fn oversized_payload_is_denied() {
        let req = TransitRequest {
            from_tier: AccessTier::Public,
            to_tier: AccessTier::Restricted,
            envelope_size: MAX_TRANSIT_ENVELOPE_BYTES + 1,
        };
        assert_eq!(execute(req), Err(TransitErr::OversizedPayload));
    }

    #[test]
    fn valid_transit_returns_id() {
        let req = TransitRequest {
            from_tier: AccessTier::Public,
            to_tier: AccessTier::Secret,
            envelope_size: 4096,
        };
        let id = execute(req).expect("valid transit must succeed under standing quintuple-auth");
        assert!(id >= 1);
    }

    #[test]
    fn transit_id_increments() {
        let req = TransitRequest {
            from_tier: AccessTier::Public,
            to_tier: AccessTier::Hidden,
            envelope_size: 256,
        };
        let a = execute(req).expect("first transit must succeed");
        let b = execute(req).expect("second transit must succeed");
        assert!(b > a, "transit-id must strictly increase: a={} b={}", a, b);
    }
}
