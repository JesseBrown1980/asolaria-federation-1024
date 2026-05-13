//! BEHCS-1024 bus fabric · Phase-5 Steps 81-100
//!
//! The kernel-side portion of the bus: alphabet binding + prefix-tree dispatch.
//! Real envelope queue is `envelope/mod.rs` (lock-free MPMC). This module gives
//! semantic meaning to RouteHint via the BEHCS-1024 1024-glyph alphabet.
//!
//! Per microkernel review verdict (Vanguard scout #1): this module IS kernel-mode
//! since it sits in the IPC path. Storage of the alphabet table is just static const.
//!
//! Cross-references:
//!   - `liris-bus-bias-receipts:82249` (Phase-5 invariants 4 hard-zero + 2 bounded)
//!   - `envelope/mod.rs` (Step 29 ring buffer + RouteHint)

use alloc::vec::Vec;

/// BEHCS-1024 alphabet size — 1024 glyphs per canon (vs legacy 256).
pub const BEHCS1024_ALPHABET_SIZE: u32 = 1024;

/// Prefix-tree branching factor (matches alphabet size per Brown-Hilbert canon).
pub const PREFIX_TREE_BRANCH: u32 = BEHCS1024_ALPHABET_SIZE;

/// Maximum depth of the prefix-tree address (port-as-label depth K).
pub const PREFIX_TREE_MAX_DEPTH: u32 = 20;

/// Addressable label-space ceiling: 1024^20 ≈ 1.2 × 10^60.
/// (Not all populated — sparse by definition.)
pub const ADDRESSABLE_CEILING_LOG10: u32 = 60;

/// Bus fabric errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BusFabricErr {
    /// Label exceeds max-depth.
    LabelTooDeep,
    /// Glyph index out of alphabet bounds (≥ 1024).
    GlyphOutOfRange,
    /// Prefix-tree walk hit unallocated branch (sparse path).
    BranchEmpty,
    /// Stub not yet implemented (Phase-5 wave).
    Unimplemented,
}

/// A port-as-label tuple — N-dimensional address per Brown-Hilbert prefix-tree.
/// Maximum K=20 levels deep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortLabel {
    /// Glyph indices at each level (most-significant first).
    pub levels: Vec<u16>,
}

impl PortLabel {
    /// Constructs a port label from level glyph indices.
    pub fn from_levels(levels: &[u16]) -> Result<Self, BusFabricErr> {
        if levels.len() as u32 > PREFIX_TREE_MAX_DEPTH {
            return Err(BusFabricErr::LabelTooDeep);
        }
        for &g in levels {
            if g as u32 >= BEHCS1024_ALPHABET_SIZE {
                return Err(BusFabricErr::GlyphOutOfRange);
            }
        }
        Ok(Self {
            levels: levels.to_vec(),
        })
    }

    /// Returns the depth (number of levels).
    pub fn depth(&self) -> usize {
        self.levels.len()
    }
}

/// Per-tier bus throughput targets (per REPO_LAW Invariant 7 + KERNEL_TARGETS.md).
pub mod throughput {
    pub const T1_MICRO_ENVELOPES_PER_SEC: u32 = 100;
    pub const T2_COSIGN_ENVELOPES_PER_SEC: u32 = 10;
    pub const T3_FIRMWARE_ENVELOPES_PER_SEC: u32 = 1;
    /// gc-trigger every N envelopes (matches BEHCS-2000-msg gulp scaled).
    pub const GC_TRIGGER_EVERY_N_ENVELOPES: u32 = 2000;
}

/// Resolve a port label to a destination handle via O(K) prefix-walk.
/// v0.1 stub.
pub fn resolve_label(_label: &PortLabel) -> Result<u64, BusFabricErr> {
    Err(BusFabricErr::Unimplemented)
}

/// Reverse-resolve a destination handle back to its port label.
/// v0.1 stub.
pub fn label_for_handle(_handle: u64) -> Result<PortLabel, BusFabricErr> {
    Err(BusFabricErr::Unimplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_is_1024() {
        assert_eq!(BEHCS1024_ALPHABET_SIZE, 1024);
    }

    #[test]
    fn max_depth_is_20() {
        assert_eq!(PREFIX_TREE_MAX_DEPTH, 20);
    }

    #[test]
    fn ceiling_log10_is_60() {
        assert_eq!(ADDRESSABLE_CEILING_LOG10, 60);
    }

    #[test]
    fn port_label_from_levels_ok() {
        let p = PortLabel::from_levels(&[1, 2, 3]).unwrap();
        assert_eq!(p.depth(), 3);
    }

    #[test]
    fn port_label_too_deep_rejected() {
        let levels: Vec<u16> = (0..21).map(|i| i as u16).collect();
        assert_eq!(
            PortLabel::from_levels(&levels),
            Err(BusFabricErr::LabelTooDeep)
        );
    }

    #[test]
    fn port_label_glyph_out_of_range() {
        assert_eq!(
            PortLabel::from_levels(&[1024]),
            Err(BusFabricErr::GlyphOutOfRange)
        );
    }

    #[test]
    fn throughput_targets_match_canon() {
        assert_eq!(throughput::T1_MICRO_ENVELOPES_PER_SEC, 100);
        assert_eq!(throughput::T2_COSIGN_ENVELOPES_PER_SEC, 10);
        assert_eq!(throughput::T3_FIRMWARE_ENVELOPES_PER_SEC, 1);
        assert_eq!(throughput::GC_TRIGGER_EVERY_N_ENVELOPES, 2000);
    }
}
