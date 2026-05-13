//! Hookwall primitive · Phase-3 advance prep (Step 41-60)
//!
//! Per REPO_LAW Invariant 2: hookwall fires on every syscall. Pre and post hooks emit
//! verdicts (PROCEED/HOLD/BLOCK) which append to cosign-chain (Invariant 4).
//!
//! Reservation: **slots 0-63** for syscall-table hookwall gates — aligns with
//!   - aether `AETHER_PHASE_2_FIRST_ARTIFACT :81580` (syscall slots 0-63 hookwall reservation)
//!   - liris `LIRIS_HOOKWALL_BOUND_CHECK_LANDED` cycle3 (Phase-3 prep slots 0-63)
//!
//! v0.1 stub: API surface only. Real verdict-emission + tier-gate enforcement lands in Phase-3.

use crate::syscall::{AccessTier, HookwallVerdict, SyscallErr};

/// Reserved hookwall slot count per federation cross-vantage agreement.
/// Slots 0-15 map 1:1 to canonical 16 syscalls; slots 16-63 reserved for Phase-3 expansion (tier-2 cosign required).
pub const HOOKWALL_SLOT_COUNT: usize = 64;

/// Tier-class of a hookwall hook. Maps to REPO_LAW Invariant 5 access-tier matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTier {
    /// Tier-1 micro: T1 envelopes, auto-allow if signature valid.
    Micro,
    /// Tier-2 cosign: requires cosign-chain row before PROCEED.
    Cosign,
    /// Tier-3 firmware: requires quintet cosign + operator-witness.
    Firmware,
}

/// Verdict emission context — passed to pre-exec hooks.
#[derive(Debug, Clone, Copy)]
pub struct HookContext {
    /// Hookwall slot index (0-63).
    pub slot: u8,
    /// Syscall number being hooked.
    pub syscall_no: u8,
    /// Caller PID (BEHCS-1024 60-tuple).
    pub caller_pid: u64,
    /// Tier-class of this hook.
    pub tier: HookTier,
    /// Access tier of the target resource.
    pub target_tier: AccessTier,
}

/// Per-tier-class throughput targets per REPO_LAW Invariant 7 (Bus Fabric throughput canon).
pub mod throughput_targets {
    /// T1 micro: ≥ 100 envelopes/sec sustained.
    pub const T1_MICRO_ENVELOPES_PER_SEC: u32 = 100;
    /// T2 cosign: ≥ 10 envelopes/sec sustained.
    pub const T2_COSIGN_ENVELOPES_PER_SEC: u32 = 10;
    /// T3 firmware: ≥ 1 envelope/sec sustained.
    pub const T3_FIRMWARE_ENVELOPES_PER_SEC: u32 = 1;
    /// Hookwall overall benchmark target per Phase-3 Step 58.
    pub const HOOKWALL_BENCHMARK_OPS_PER_SEC: u32 = 100_000;
}

/// Pre-exec hookwall stub.
/// v0.1: returns Proceed for Micro, Hold for Cosign, Block for Firmware (conservative defaults).
/// Real impl in Phase-3 reads `hookwall-policy.json` and consults GNN gate.
pub fn hookwall_pre(ctx: &HookContext) -> Result<HookwallVerdict, SyscallErr> {
    if ctx.slot as usize >= HOOKWALL_SLOT_COUNT {
        return Err(SyscallErr::Invalid);
    }
    Ok(match ctx.tier {
        HookTier::Micro => HookwallVerdict::Proceed,
        HookTier::Cosign => HookwallVerdict::Hold,
        HookTier::Firmware => HookwallVerdict::Block,
    })
}

/// Post-exec hookwall stub.
/// v0.1.6: validates slot bounds + accepts any verdict locally (no cosign-chain append
/// until Phase-3 wires append-only ndjson writer). Returns Ok(()) on valid context.
/// Verdict::Block is observational here — caller decided pre-exec; post-hook merely records.
pub fn hookwall_post(ctx: &HookContext, _verdict: HookwallVerdict) -> Result<(), SyscallErr> {
    if ctx.slot as usize >= HOOKWALL_SLOT_COUNT {
        return Err(SyscallErr::Invalid);
    }
    Ok(())
}

/// Cross-vantage agreement marker — embedded so future peers can grep for this constant
/// and confirm acer's reservation matches theirs.
pub const FEDERATION_SLOT_RESERVATION_TAG: &str =
    "HOOKWALL_SLOTS_0_TO_63_RESERVED_BY_ACER_LIRIS_AETHER_PHASE_2_CONVERGENT";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscall::HookwallVerdict;

    #[test]
    fn slot_count_is_64() {
        assert_eq!(HOOKWALL_SLOT_COUNT, 64);
    }

    #[test]
    fn micro_tier_pre_proceeds() {
        let ctx = HookContext {
            slot: 0,
            syscall_no: 1,
            caller_pid: 0,
            tier: HookTier::Micro,
            target_tier: AccessTier::Public,
        };
        assert_eq!(hookwall_pre(&ctx), Ok(HookwallVerdict::Proceed));
    }

    #[test]
    fn cosign_tier_pre_holds() {
        let ctx = HookContext {
            slot: 0,
            syscall_no: 1,
            caller_pid: 0,
            tier: HookTier::Cosign,
            target_tier: AccessTier::Restricted,
        };
        assert_eq!(hookwall_pre(&ctx), Ok(HookwallVerdict::Hold));
    }

    #[test]
    fn firmware_tier_pre_blocks() {
        let ctx = HookContext {
            slot: 0,
            syscall_no: 1,
            caller_pid: 0,
            tier: HookTier::Firmware,
            target_tier: AccessTier::Secret,
        };
        assert_eq!(hookwall_pre(&ctx), Ok(HookwallVerdict::Block));
    }

    #[test]
    fn slot_out_of_range_returns_invalid() {
        let ctx = HookContext {
            slot: 64, // out of range (0-63)
            syscall_no: 1,
            caller_pid: 0,
            tier: HookTier::Micro,
            target_tier: AccessTier::Public,
        };
        assert_eq!(hookwall_pre(&ctx), Err(SyscallErr::Invalid));
    }
}
