//! Atlas Module · BEHCS-1024 codepoint → domain/supervisor/lane lookup
//!
//! Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
//! Atlas source: `D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson`
//! (1024 lines, one per cp, schema `{cp, name, domain, supervisor, source}`).
//!
//! Per `kernel/docs/ATLAS_INTEGRATION.md` + Foundation v2 LAW, the cp → domain
//! mapping is SEALED with quintuple cosign IN-WRITING. Ranges are therefore
//! stable and encoded as `match cp` arms here — no runtime ndjson load is
//! required for syscall-level wiring.
//!
//! Authored cycle-69 under standing two-week auth (Atlas-Module assignment).
//! Reserved-role lookup (`reserved_role_for_cp`) is deferred to Phase-9 when
//! `pid::classify_subclass` HookwallCp dispatch needs it.
//!
//! Domain table (verified against atlas-index.ndjson boundaries):
//!
//! | cp range  | domain              | supervisor          | lane        |
//! |-----------|---------------------|---------------------|-------------|
//! | 0-255     | legacy_subset_256   | cube_cubed_sealer   | memory      |
//! | 256-383   | sovereignty         | gaia                | skeletal    |
//! | 384-479   | sequencing          | helm                | muscular    |
//! | 480-575   | language_glyph      | vector              | nervous     |
//! | 576-703   | hilbert_cube        | rook                | circulatory |
//! | 704-799   | build_proof         | forge               | muscular    |
//! | 800-895   | federation          | falcon              | nervous     |
//! | 896-1023  | emergent_residual   | livefree            | memory      |

/// One-past-end of the BEHCS-1024 codepoint range. Valid cps are `0..BEHCS_1024_CP_COUNT`.
pub const BEHCS_1024_CP_COUNT: u16 = 1024;

/// Inclusive max valid cp. Convenience alias = `BEHCS_1024_CP_COUNT - 1`.
pub const BEHCS_1024_MAX_CP: u16 = BEHCS_1024_CP_COUNT - 1;

/// One-past-end of the BEHCS-256 subset range. Cps `0..BEHCS_256_CP_COUNT` are the
/// legacy-subset band that BEHCS-1024 must inherit semantics from per the
/// subset-embedding invariant.
pub const BEHCS_256_CP_COUNT: u16 = 256;

/// Returns `true` iff `cp` is a valid BEHCS-1024 codepoint (i.e. `cp < 1024`).
/// `const fn` — usable in syscall-tier no-alloc paths.
pub const fn is_valid_cp(cp: u16) -> bool {
    cp < BEHCS_1024_CP_COUNT
}

/// Map a BEHCS-1024 cp to its BEHCS-256 subset-embedding equivalent.
///
/// Per the subset-embedding invariant: cps 256-1023 inherit semantics from `cp mod 256`.
/// Cps 0-255 are the identity. Out-of-range cps return `None`.
///
/// This is the load-bearing function for backward-compat: any BEHCS-256 consumer that
/// receives a BEHCS-1024 cp should call this to obtain the legacy interpretation.
/// `const fn` — usable in syscall-tier no-alloc paths.
pub const fn subset_embedding_cp(cp: u16) -> Option<u8> {
    if cp >= BEHCS_1024_CP_COUNT {
        None
    } else {
        // cp < 1024, so cp % 256 ∈ 0..=255 fits in u8 losslessly.
        Some((cp % BEHCS_256_CP_COUNT) as u8)
    }
}

/// Atlas domain for a BEHCS-1024 codepoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtlasDomain {
    /// cp 0-255 · BEHCS-256 legacy subset embedding.
    LegacySubset256,
    /// cp 256-383 · operator-class sovereignty band.
    Sovereignty,
    /// cp 384-479 · sequencing / ordering primitives.
    Sequencing,
    /// cp 480-575 · language-glyph band.
    LanguageGlyph,
    /// cp 576-703 · Hilbert-cube spatial band.
    HilbertCube,
    /// cp 704-799 · build / proof band.
    BuildProof,
    /// cp 800-895 · federation / cross-device band.
    Federation,
    /// cp 896-1023 · emergent residual band.
    EmergentResidual,
}

/// Atlas supervisor for a BEHCS-1024 codepoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtlasSupervisor {
    /// Supervisor for `LegacySubset256`.
    CubeCubedSealer,
    /// Supervisor for `Sovereignty`.
    Gaia,
    /// Supervisor for `Sequencing`.
    Helm,
    /// Supervisor for `LanguageGlyph`.
    Vector,
    /// Supervisor for `HilbertCube`.
    Rook,
    /// Supervisor for `BuildProof`.
    Forge,
    /// Supervisor for `Federation`.
    Falcon,
    /// Supervisor for `EmergentResidual`.
    LiveFree,
}

/// Atlas lane for a BEHCS-1024 codepoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtlasLane {
    /// Memory lane (legacy_subset_256, emergent_residual).
    Memory,
    /// Skeletal lane (sovereignty).
    Skeletal,
    /// Muscular lane (sequencing, build_proof).
    Muscular,
    /// Nervous lane (language_glyph, federation).
    Nervous,
    /// Circulatory lane (hilbert_cube).
    Circulatory,
}

/// Return the atlas domain for `cp`, or `None` if `cp` falls outside 0-1023.
pub fn domain_for_cp(cp: u16) -> Option<AtlasDomain> {
    match cp {
        0..=255 => Some(AtlasDomain::LegacySubset256),
        256..=383 => Some(AtlasDomain::Sovereignty),
        384..=479 => Some(AtlasDomain::Sequencing),
        480..=575 => Some(AtlasDomain::LanguageGlyph),
        576..=703 => Some(AtlasDomain::HilbertCube),
        704..=799 => Some(AtlasDomain::BuildProof),
        800..=895 => Some(AtlasDomain::Federation),
        896..=1023 => Some(AtlasDomain::EmergentResidual),
        _ => None,
    }
}

/// Return the atlas supervisor for `cp`, or `None` if `cp` falls outside 0-1023.
pub fn supervisor_for_cp(cp: u16) -> Option<AtlasSupervisor> {
    match cp {
        0..=255 => Some(AtlasSupervisor::CubeCubedSealer),
        256..=383 => Some(AtlasSupervisor::Gaia),
        384..=479 => Some(AtlasSupervisor::Helm),
        480..=575 => Some(AtlasSupervisor::Vector),
        576..=703 => Some(AtlasSupervisor::Rook),
        704..=799 => Some(AtlasSupervisor::Forge),
        800..=895 => Some(AtlasSupervisor::Falcon),
        896..=1023 => Some(AtlasSupervisor::LiveFree),
        _ => None,
    }
}

/// Return the atlas lane for `cp`, or `None` if `cp` falls outside 0-1023.
pub fn lane_for_cp(cp: u16) -> Option<AtlasLane> {
    match cp {
        0..=255 => Some(AtlasLane::Memory),
        256..=383 => Some(AtlasLane::Skeletal),
        384..=479 => Some(AtlasLane::Muscular),
        480..=575 => Some(AtlasLane::Nervous),
        576..=703 => Some(AtlasLane::Circulatory),
        704..=799 => Some(AtlasLane::Muscular),
        800..=895 => Some(AtlasLane::Nervous),
        896..=1023 => Some(AtlasLane::Memory),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_for_cp_legacy_range_0_to_255() {
        assert_eq!(domain_for_cp(0), Some(AtlasDomain::LegacySubset256));
        assert_eq!(domain_for_cp(128), Some(AtlasDomain::LegacySubset256));
        assert_eq!(domain_for_cp(255), Some(AtlasDomain::LegacySubset256));
        // Boundary: 256 must NOT be legacy.
        assert_ne!(domain_for_cp(256), Some(AtlasDomain::LegacySubset256));
    }

    #[test]
    fn domain_for_cp_sovereignty_range_256_to_383() {
        assert_eq!(domain_for_cp(256), Some(AtlasDomain::Sovereignty));
        // cp 260 = gaia-Jesse-Daniel-Brown-apex; cp 261 = gaia-Rayssa-Chiqueto-apex.
        assert_eq!(domain_for_cp(260), Some(AtlasDomain::Sovereignty));
        assert_eq!(domain_for_cp(261), Some(AtlasDomain::Sovereignty));
        assert_eq!(domain_for_cp(383), Some(AtlasDomain::Sovereignty));
        // Boundary: 384 must NOT be sovereignty.
        assert_ne!(domain_for_cp(384), Some(AtlasDomain::Sovereignty));
        // Supervisor + lane co-vary in this band.
        assert_eq!(supervisor_for_cp(260), Some(AtlasSupervisor::Gaia));
        assert_eq!(lane_for_cp(260), Some(AtlasLane::Skeletal));
    }

    #[test]
    fn domain_for_cp_emergent_residual_range_896_to_1023() {
        assert_eq!(domain_for_cp(896), Some(AtlasDomain::EmergentResidual));
        assert_eq!(domain_for_cp(960), Some(AtlasDomain::EmergentResidual));
        assert_eq!(domain_for_cp(1023), Some(AtlasDomain::EmergentResidual));
        // Supervisor + lane co-vary.
        assert_eq!(supervisor_for_cp(1023), Some(AtlasSupervisor::LiveFree));
        assert_eq!(lane_for_cp(1023), Some(AtlasLane::Memory));
    }

    #[test]
    fn domain_for_cp_returns_none_above_1023() {
        assert_eq!(domain_for_cp(1024), None);
        assert_eq!(domain_for_cp(2048), None);
        assert_eq!(domain_for_cp(u16::MAX), None);
        // All three accessors must agree on out-of-range.
        assert_eq!(supervisor_for_cp(1024), None);
        assert_eq!(lane_for_cp(1024), None);
    }

    // ===== cp validity + subset-embedding tests =====

    #[test]
    fn behcs_1024_constants_are_canonical() {
        assert_eq!(BEHCS_1024_CP_COUNT, 1024);
        assert_eq!(BEHCS_1024_MAX_CP, 1023);
        assert_eq!(BEHCS_256_CP_COUNT, 256);
    }

    #[test]
    fn is_valid_cp_accepts_full_range() {
        assert!(is_valid_cp(0));
        assert!(is_valid_cp(1));
        assert!(is_valid_cp(255));
        assert!(is_valid_cp(256));
        assert!(is_valid_cp(512));
        assert!(is_valid_cp(1023));
    }

    #[test]
    fn is_valid_cp_rejects_out_of_range() {
        assert!(!is_valid_cp(1024));
        assert!(!is_valid_cp(2048));
        assert!(!is_valid_cp(u16::MAX));
    }

    #[test]
    fn is_valid_cp_is_const() {
        // Compile-time evaluation — assigning into a const proves const-fn-ness.
        const VALID_ZERO: bool = is_valid_cp(0);
        const VALID_MAX: bool = is_valid_cp(BEHCS_1024_MAX_CP);
        const VALID_OOB: bool = is_valid_cp(BEHCS_1024_CP_COUNT);
        assert!(VALID_ZERO);
        assert!(VALID_MAX);
        assert!(!VALID_OOB);
    }

    #[test]
    fn subset_embedding_identity_on_legacy_band() {
        // cps 0-255 are the identity.
        assert_eq!(subset_embedding_cp(0), Some(0));
        assert_eq!(subset_embedding_cp(1), Some(1));
        assert_eq!(subset_embedding_cp(127), Some(127));
        assert_eq!(subset_embedding_cp(255), Some(255));
    }

    #[test]
    fn subset_embedding_inherits_from_cp_mod_256() {
        // cps 256-1023 inherit semantics from cp mod 256.
        assert_eq!(subset_embedding_cp(256), Some(0)); // cp 256 → BEHCS-256 0
        assert_eq!(subset_embedding_cp(257), Some(1));
        assert_eq!(subset_embedding_cp(383), Some(127));
        assert_eq!(subset_embedding_cp(512), Some(0)); // cp 512 → BEHCS-256 0 (second cycle)
        assert_eq!(subset_embedding_cp(768), Some(0)); // cp 768 → BEHCS-256 0 (third cycle)
        assert_eq!(subset_embedding_cp(1023), Some(255));
    }

    #[test]
    fn subset_embedding_rejects_out_of_range() {
        assert_eq!(subset_embedding_cp(1024), None);
        assert_eq!(subset_embedding_cp(u16::MAX), None);
    }

    #[test]
    fn subset_embedding_invariant_holds_for_full_range() {
        // Property: for any valid cp, subset_embedding_cp(cp).unwrap() == (cp % 256) as u8.
        // This is the core invariant; this test runs the full 0..1024 range.
        for cp in 0..BEHCS_1024_CP_COUNT {
            assert_eq!(
                subset_embedding_cp(cp),
                Some((cp % BEHCS_256_CP_COUNT) as u8),
                "subset embedding broken at cp {}",
                cp
            );
        }
    }

    #[test]
    fn subset_embedding_is_const() {
        const E_ZERO: Option<u8> = subset_embedding_cp(0);
        const E_FOUR_HUNDRED: Option<u8> = subset_embedding_cp(400);
        const E_OOB: Option<u8> = subset_embedding_cp(1024);
        assert_eq!(E_ZERO, Some(0));
        assert_eq!(E_FOUR_HUNDRED, Some(144));
        assert_eq!(E_OOB, None);
    }
}
