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

/// BEHCS-1024 alphabet table — Phase-1 identity mapping (idx → idx as u32).
/// Phase-2 will replace with real glyph-codepoint table from
/// `data/behcs/codex/alphabet-1024.json`. Public API surface does not change.
pub const BEHCS1024_ALPHABET_TABLE: [u32; 1024] = {
    let mut t = [0u32; 1024];
    let mut i = 0usize;
    while i < 1024 {
        t[i] = i as u32;
        i += 1;
    }
    t
};

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

    /// Canonical byte serialization — big-endian u16 (2 bytes per level).
    /// Phase-1 stable byte contract for FNV-1a-64 hashing. Phase-2 will not
    /// change this byte layout (the alphabet swap only changes table values,
    /// not the encoding of label glyph indices into bytes).
    pub fn canonical_glyph_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.levels.len() * 2);
        for &g in &self.levels {
            out.push((g >> 8) as u8);
            out.push((g & 0xFF) as u8);
        }
        out
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

/// FNV-1a-64 over a byte slice. Constant-time per-input-length (not data-
/// dependent branches). Used by `resolve_label` to produce a stable u64
/// destination handle from canonical glyph bytes.
///
/// Phase-1 placeholder using FNV-1a-64. Phase-2 may swap to a wider Brown-
/// Hilbert prefix-tree allocator; the public `resolve_label` signature does
/// not change.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Reserved handle: zero is never returned for a non-empty label. Returned by
/// `label_for_handle` semantics as the "no such handle" sentinel domain.
pub const RESERVED_HANDLE_ZERO: u64 = 0;

/// Resolve a port label to a destination handle via O(K) prefix-walk.
///
/// Phase-1 placeholder using FNV-1a-64 over canonical glyph bytes. Phase-2
/// will replace with real Brown-Hilbert prefix-tree allocator (deferred until
/// allocator design lands). Deterministic, host-independent, byte-stable.
pub fn resolve_label(label: &PortLabel) -> Result<u64, BusFabricErr> {
    if label.depth() as u32 > PREFIX_TREE_MAX_DEPTH {
        return Err(BusFabricErr::LabelTooDeep);
    }
    for &g in &label.levels {
        if g as u32 >= BEHCS1024_ALPHABET_SIZE {
            return Err(BusFabricErr::GlyphOutOfRange);
        }
    }
    let bytes = label.canonical_glyph_bytes();
    let mut h = fnv1a_64(&bytes);
    // Reserve zero — fold to a nonzero domain for non-empty bytes.
    if h == RESERVED_HANDLE_ZERO && !bytes.is_empty() {
        h = 0x8000_0000_0000_0000;
    }
    Ok(h)
}

/// O(K) prefix-tree walker — Phase-1 parity-stub that matches `resolve_label`.
///
/// Phase-1 placeholder: walks levels in order and feeds FNV-1a-64. The walk
/// is O(K) where K = label.depth() ≤ PREFIX_TREE_MAX_DEPTH. Phase-2 will
/// replace with branch-table descent producing identical u64 for identical
/// canonical bytes (parity guaranteed by the dedicated test).
pub fn prefix_tree_walk(label: &PortLabel) -> Result<u64, BusFabricErr> {
    resolve_label(label)
}

/// Reverse-resolve a destination handle back to its port label.
///
/// Phase-1 placeholder: returns `BranchEmpty` for all handles (the FNV-1a-64
/// forward map is intentionally non-invertible at this phase). Phase-2 will
/// maintain a reverse-index alongside the prefix-tree allocator. Signature
/// preserved verbatim across phases.
pub fn label_for_handle(_handle: u64) -> Result<PortLabel, BusFabricErr> {
    Err(BusFabricErr::BranchEmpty)
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

    #[test]
    fn resolve_label_is_deterministic() {
        let p = PortLabel::from_levels(&[1, 2, 3, 4]).unwrap();
        let h1 = resolve_label(&p).unwrap();
        let h2 = resolve_label(&p).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn resolve_label_distinct_labels_differ() {
        let a = PortLabel::from_levels(&[1, 2, 3]).unwrap();
        let b = PortLabel::from_levels(&[1, 2, 4]).unwrap();
        assert_ne!(resolve_label(&a).unwrap(), resolve_label(&b).unwrap());
    }

    #[test]
    fn canonical_glyph_bytes_big_endian_u16() {
        // 0x0102 → [0x01, 0x02]; 0x03FF → [0x03, 0xFF]
        let p = PortLabel::from_levels(&[0x0102, 0x03FF]).unwrap();
        let bytes = p.canonical_glyph_bytes();
        assert_eq!(bytes, alloc::vec![0x01u8, 0x02u8, 0x03u8, 0xFFu8]);
    }

    #[test]
    fn alphabet_table_is_identity_phase1() {
        assert_eq!(BEHCS1024_ALPHABET_TABLE[0], 0);
        assert_eq!(BEHCS1024_ALPHABET_TABLE[1], 1);
        assert_eq!(BEHCS1024_ALPHABET_TABLE[42], 42);
        assert_eq!(BEHCS1024_ALPHABET_TABLE[1023], 1023);
    }

    #[test]
    fn alphabet_table_length_is_1024() {
        assert_eq!(
            BEHCS1024_ALPHABET_TABLE.len() as u32,
            BEHCS1024_ALPHABET_SIZE
        );
    }

    #[test]
    fn prefix_tree_walk_parity_with_resolve_label() {
        let labels: [&[u16]; 4] = [&[0], &[7, 8], &[1, 2, 3], &[100, 200, 300, 400, 500]];
        for &lv in &labels {
            let p = PortLabel::from_levels(lv).unwrap();
            assert_eq!(resolve_label(&p).unwrap(), prefix_tree_walk(&p).unwrap());
        }
    }

    #[test]
    fn label_for_handle_phase1_returns_branch_empty() {
        assert_eq!(label_for_handle(0), Err(BusFabricErr::BranchEmpty));
        assert_eq!(
            label_for_handle(0xDEAD_BEEF_CAFE_F00D),
            Err(BusFabricErr::BranchEmpty)
        );
    }

    #[test]
    fn resolve_label_handle_zero_reserved_for_nonempty() {
        // Phase-1 contract: zero handle is reserved; non-empty labels never
        // map to RESERVED_HANDLE_ZERO. Probe a spread of labels.
        for i in 0..256u16 {
            let p = PortLabel::from_levels(&[i]).unwrap();
            assert_ne!(resolve_label(&p).unwrap(), RESERVED_HANDLE_ZERO);
        }
    }
}
