//! Spawn gate ring — composes the white-room gate primitives into ONE verdict for a spawn request,
//! evaluated BEFORE any (gated) launch path. Composes, strictest-wins:
//!   1. sign_gate::deny_check — banned event tags (HD-virus / drift / malformed-PID / …) -> BLOCK.
//!   2. hookwall tier verdict — Micro -> Proceed-candidate, Cosign -> HOLD, Firmware -> BLOCK.
//!   3. reverse-gain (the Omniflywheel rule) — forward score must clear the genius threshold AND the
//!      reverse-gain risk must be within bound, else HOLD.
//!
//! PURE / E=0: this computes a verdict ONLY. It does NOT append to the cosign chain — the append-only
//! SEAL (hookwall_post -> cosign_chain::append) is a separate, GATED, fire-time step. `seal_row`
//! composes the 16-byte HVD witness bytes (still E=0); appending them is the caller's gated job.

use crate::hookwall::{hookwall_pre, HookContext, HookTier};
use crate::sign_gate::{deny_check, DEFAULT_DENIES};
use crate::syscall::{AccessTier, HookwallVerdict};

/// Genius threshold (quantized 0..1000) — the forward score must clear this (== 0.72), matching the
/// gnn-oracle white-room / Omniflywheel `WHITEROOM_GENIUS_THRESHOLD`.
pub const GATE_GENIUS_THRESHOLD_Q: u32 = 720;

/// Maximum reverse-gain risk (quantized 0..1000) the gate permits (== 0.28), matching the gnn-oracle
/// `OMNIFLYWHEEL_MAX_REVERSE_RISK`.
pub const GATE_MAX_REVERSE_RISK_Q: u32 = 280;

/// Inputs to the spawn gate ring. Scores are quantized 0..1000 (fixed-point, no float — cross-runtime
/// parity, matching `score_frame_q`).
pub struct SpawnGateInput<'a> {
    /// Hookwall slot (0..64).
    pub slot: u8,
    /// Syscall number being gated.
    pub syscall_no: u8,
    /// Caller PID (BEHCS-1024 60-tuple, low 64 bits).
    pub caller_pid: u64,
    /// Tier-class of the spawn request.
    pub tier: HookTier,
    /// Access tier of the target resource.
    pub target_tier: AccessTier,
    /// Hookwall event tags screened against `DEFAULT_DENIES`.
    pub event_tags: &'a [&'a str],
    /// Forward (genius) score, quantized 0..1000.
    pub forward_score_q: u32,
    /// Reverse-gain risk, quantized 0..1000.
    pub reverse_risk_q: u32,
}

/// Compose the gate ring into a single verdict. BLOCK > HOLD > PROCEED (strictest wins).
pub fn spawn_gate_verdict(input: &SpawnGateInput<'_>) -> HookwallVerdict {
    // 1. sign_gate: any banned tag -> BLOCK (deny_check returns false when a deny term hits).
    if !deny_check(input.event_tags, DEFAULT_DENIES) {
        return HookwallVerdict::Block;
    }
    // 2. hookwall tier gate (Micro -> Proceed / Cosign -> Hold / Firmware -> Block).
    let ctx = HookContext {
        slot: input.slot,
        syscall_no: input.syscall_no,
        caller_pid: input.caller_pid,
        tier: input.tier,
        target_tier: input.target_tier,
    };
    let tier_verdict = match hookwall_pre(&ctx) {
        Ok(verdict) => verdict,
        // Out-of-range slot etc. -> conservative BLOCK (never default-open).
        Err(_) => return HookwallVerdict::Block,
    };
    if tier_verdict != HookwallVerdict::Proceed {
        return tier_verdict;
    }
    // 3. reverse-gain (Omniflywheel): a clean, tier-cleared request STILL waits on the score gate —
    //    forward score must clear genius AND reverse risk must be within bound, else HOLD.
    if input.forward_score_q >= GATE_GENIUS_THRESHOLD_Q
        && input.reverse_risk_q <= GATE_MAX_REVERSE_RISK_Q
    {
        HookwallVerdict::Proceed
    } else {
        HookwallVerdict::Hold
    }
}

/// Compose the 16-byte append-only HVD seal row for a gate verdict (reuses
/// `hookwall::compose_verdict_row`). PURE / E=0: composes bytes only — it does NOT append to the
/// cosign chain. The actual append (the audit SEAL) is `hookwall_post`, a GATED fire-time step.
pub fn seal_row(input: &SpawnGateInput<'_>, verdict: HookwallVerdict) -> [u8; 16] {
    let ctx = HookContext {
        slot: input.slot,
        syscall_no: input.syscall_no,
        caller_pid: input.caller_pid,
        tier: input.tier,
        target_tier: input.target_tier,
    };
    crate::hookwall::compose_verdict_row(&ctx, verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(tags: &'a [&'a str], tier: HookTier, fwd: u32, rev: u32) -> SpawnGateInput<'a> {
        SpawnGateInput {
            slot: 0,
            syscall_no: 1,
            caller_pid: 0xABCD,
            tier,
            target_tier: AccessTier::Public,
            event_tags: tags,
            forward_score_q: fwd,
            reverse_risk_q: rev,
        }
    }

    #[test]
    fn thresholds_match_omniflywheel() {
        assert_eq!(GATE_GENIUS_THRESHOLD_Q, 720);
        assert_eq!(GATE_MAX_REVERSE_RISK_Q, 280);
    }

    #[test]
    fn clean_micro_high_score_low_risk_proceeds() {
        let tags = ["safe-tag", "all-clear"];
        assert_eq!(
            spawn_gate_verdict(&input(&tags, HookTier::Micro, 800, 100)),
            HookwallVerdict::Proceed
        );
    }

    #[test]
    fn banned_tag_blocks_even_if_everything_else_passes() {
        let tags = ["work", "drift-detected"]; // "drift" is in DEFAULT_DENIES
        assert_eq!(
            spawn_gate_verdict(&input(&tags, HookTier::Micro, 999, 0)),
            HookwallVerdict::Block
        );
    }

    #[test]
    fn firmware_blocks_and_cosign_holds_regardless_of_score() {
        let tags = ["safe-tag"];
        assert_eq!(
            spawn_gate_verdict(&input(&tags, HookTier::Firmware, 999, 0)),
            HookwallVerdict::Block
        );
        assert_eq!(
            spawn_gate_verdict(&input(&tags, HookTier::Cosign, 999, 0)),
            HookwallVerdict::Hold
        );
    }

    #[test]
    fn clean_micro_but_weak_score_or_high_risk_holds() {
        let tags = ["safe-tag"];
        // forward score below genius threshold -> HOLD
        assert_eq!(
            spawn_gate_verdict(&input(&tags, HookTier::Micro, 700, 100)),
            HookwallVerdict::Hold
        );
        // reverse risk above bound -> HOLD
        assert_eq!(
            spawn_gate_verdict(&input(&tags, HookTier::Micro, 800, 300)),
            HookwallVerdict::Hold
        );
        // exactly at both bounds -> PROCEED
        assert_eq!(
            spawn_gate_verdict(&input(
                &tags,
                HookTier::Micro,
                GATE_GENIUS_THRESHOLD_Q,
                GATE_MAX_REVERSE_RISK_Q
            )),
            HookwallVerdict::Proceed
        );
    }

    #[test]
    fn out_of_range_slot_blocks_conservatively() {
        let tags = ["safe-tag"];
        let mut inp = input(&tags, HookTier::Micro, 800, 100);
        inp.slot = 64; // out of range
        assert_eq!(spawn_gate_verdict(&inp), HookwallVerdict::Block);
    }

    #[test]
    fn seal_row_is_hvd_witness_bytes() {
        let tags = ["safe-tag"];
        let inp = input(&tags, HookTier::Micro, 800, 100);
        let row = seal_row(&inp, HookwallVerdict::Proceed);
        // bytes 13..16 = "HVD" magic; this composes bytes only — nothing is appended to the chain.
        assert_eq!(&row[13..16], b"HVD");
        assert_eq!(row[3], 1); // Proceed code
    }
}
