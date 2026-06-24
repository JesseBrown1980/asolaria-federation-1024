//! Corpus access-LEVEL tagger — assigns each recall/atlas row a LEVEL so the cross-fabric gate can
//! serve a PROVABLY PII-free PUBLIC tier (level 0) while owner-trusted links reach private tiers.
//! Pairs with `link_auth::effective_level`: the gate authorizes a caller's max level; the serve
//! layer returns ONLY rows whose tagged level <= that. PURE / E=0: classification from path +
//! content; no I/O. This is the spec the live node engine mirrors (like the HMAC contract).
//!
//! Three tiers, CONSERVATIVE — PII can never fall to public, and "unknown" is NOT public:
//!   - `LEVEL_OWNER_PRIVATE` (9): legal / financial / customer / personal PII + secrets/keys/vault.
//!     Reachable only by the owner's own trusted links. Checked FIRST, so it always wins.
//!   - `LEVEL_FEDERATION`   (5): default for anything not positively cleared as public canon.
//!   - `LEVEL_PUBLIC`       (0): ONLY explicit public-canon rows (maps / addressing / reductions /
//!     public docs) that also carry no PII — the public "search engine for agents" tier.

/// Public tier — carve-out-clean, shareable to any fabric.
pub const LEVEL_PUBLIC: u8 = 0;
/// Federation-internal tier — private to the colony; not public, not necessarily PII.
pub const LEVEL_FEDERATION: u8 = 5;
/// Owner-private tier — PII / secrets; owner-trusted links only. Must never reach level 0.
pub const LEVEL_OWNER_PRIVATE: u8 = 9;

/// Path fragments that mark a row owner-private (PII / secrets), matched case-insensitively.
/// Comprehensive by necessity: when row bodies are path-derived metadata (no full text), the
/// content rules below rarely fire, so classification reduces to these path fragments. Narrow
/// "/x/" forms are de-slashed so they also catch the bare dir + filename forms (e.g. "legal"
/// catches "legal-recovery"/"Legal Analysis...docx"; ".asolaria" catches the bare secrets dir;
/// "dcim" catches "dcim.json"). Conservative: a false positive only over-privatizes (safe).
const PII_PATH_FRAGMENTS: &[&str] = &[
    // legal / financial / customer / personal docs
    "legal",
    "evidence-package",
    "evidence",
    "google-support-refund",
    "support-refund-complaints",
    "refund-complaint",
    "refund",
    "bank",
    "invoice",
    "financial",
    "paypal",
    "zelle",
    "passport",
    "cnpj",
    "cpf",
    "whatsapp-rayssa",
    // secrets / keys / vault / credentials
    "beast-keys",
    "backup-keys",
    "decrypted-vault",
    "vault",
    "charm_",
    "private-key",
    "privatekey",
    "recall.key",
    ".pem",
    ".key",
    ".pk8",
    ".kdbx",
    ".keystore",
    ".jks",
    "id_rsa",
    "id_ed25519",
    "wallet.dat",
    "seed-phrase",
    "seed_phrase",
    "mnemonic",
    "credential",
    "secret",
    "password",
    "passwd",
    ".asolaria",
    // personal-device dumps (phone DCIM / sdcard / downloads)
    "dcim",
    "sdcard",
    "falcon-dump",
    "phone-dump",
];

/// Content fragments that mark a row owner-private (financial / legal / customer PII).
const PII_CONTENT_FRAGMENTS: &[&str] = &[
    "cnpj",
    "cpf ",
    "paypal",
    "zelle",
    "refund complaint",
    "customer care",
    "passport no",
    "invoice #",
];

/// Path fragments explicitly cleared as PUBLIC canon (maps / addressing / reductions / public docs).
const PUBLIC_CANON_FRAGMENTS: &[&str] = &[
    "asolaria-multi-cylinder",
    "scientific-voxel-atlas",
    "asolaria-real-model",
    "agentterms-os-dashboard",
    "asolaria-map-index",
    "archaeology-and-significance-canon",
    "brown-hilbert",
    "what-is-asolaria",
    "algorithms-of-asolaria",
    "session-update",
    "readme",
];

#[inline]
fn ascii_lower(b: u8) -> u8 {
    if b.is_ascii_uppercase() {
        b + 32
    } else {
        b
    }
}

/// Case-insensitive substring search (ASCII). `needle_lower` MUST be lowercase. no_std, alloc-free.
fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() || n.len() > h.len() {
        return false;
    }
    let mut i = 0;
    while i + n.len() <= h.len() {
        let mut j = 0;
        while j < n.len() && ascii_lower(h[i + j]) == n[j] {
            j += 1;
        }
        if j == n.len() {
            return true;
        }
        i += 1;
    }
    false
}

/// `true` if `s` has a run of >= `min_len` consecutive ASCII digits (CNPJ/account/card-like).
fn has_digit_run(s: &str, min_len: usize) -> bool {
    let mut run = 0usize;
    for &b in s.as_bytes() {
        if b.is_ascii_digit() {
            run += 1;
            if run >= min_len {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// `true` if the row's path or content carries PII/secret material the public tier must never see.
/// Conservative: a false positive only over-privatizes a row (safe); a false negative would leak.
pub fn is_pii(path: &str, content: &str) -> bool {
    for frag in PII_PATH_FRAGMENTS {
        if contains_ci(path, frag) {
            return true;
        }
    }
    if has_digit_run(content, 14) {
        // 14+ consecutive digits — CNPJ / account / card-like.
        return true;
    }
    for frag in PII_CONTENT_FRAGMENTS {
        if contains_ci(content, frag) {
            return true;
        }
    }
    false
}

/// `true` if the row's path is explicit PUBLIC canon (maps / addressing / reductions / public docs).
pub fn is_public_canon(path: &str) -> bool {
    for frag in PUBLIC_CANON_FRAGMENTS {
        if contains_ci(path, frag) {
            return true;
        }
    }
    false
}

/// Assign a row its access level. PII wins (checked first) → never public; explicit public canon →
/// level 0; everything else → federation-private (not public). The public portal serves level 0 only.
pub fn assign_level(path: &str, content: &str) -> u8 {
    if is_pii(path, content) {
        LEVEL_OWNER_PRIVATE
    } else if is_public_canon(path) {
        LEVEL_PUBLIC
    } else {
        LEVEL_FEDERATION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_evidence_path_is_owner_private() {
        let p = "C:/Users/rayss/Asolaria/reports/legal/evidence-package/06-google-support-refund-complaints/x.md";
        assert_eq!(assign_level(p, "any content"), LEVEL_OWNER_PRIVATE);
    }

    #[test]
    fn cnpj_content_is_owner_private_even_on_a_neutral_path() {
        // A 14-digit CNPJ in the body → private regardless of path.
        assert_eq!(
            assign_level(
                "C:/x/reports/qdd.md",
                "order for CNPJ 11222333000181 placed"
            ),
            LEVEL_OWNER_PRIVATE
        );
        assert_eq!(
            assign_level("C:/x/notes.md", "paypal payment to ..."),
            LEVEL_OWNER_PRIVATE
        );
    }

    #[test]
    fn public_canon_path_is_level_public() {
        assert_eq!(
            assign_level(
                "reports/asolaria-multi-cylinder-v2.html",
                "cylinders / pipes / surfaces"
            ),
            LEVEL_PUBLIC
        );
        assert_eq!(
            assign_level("docs/what-is-asolaria.md", "addressing geometry"),
            LEVEL_PUBLIC
        );
    }

    #[test]
    fn pii_always_beats_public_canon_naming() {
        // A public-canon-NAMED file that nonetheless carries PII must NOT be public.
        assert_eq!(
            assign_level(
                "reports/asolaria-real-model.html",
                "leaked: zelle transfer ..."
            ),
            LEVEL_OWNER_PRIVATE
        );
    }

    #[test]
    fn unknown_row_is_federation_not_public() {
        assert_eq!(
            assign_level("C:/x/internal/thing.hbp", "no pii, not canon"),
            LEVEL_FEDERATION
        );
    }

    #[test]
    fn helpers_behave() {
        assert!(contains_ci("Legal/Evidence-Package", "evidence-package"));
        assert!(!contains_ci("clean/path", "evidence-package"));
        assert!(has_digit_run("abc 11222333000181 def", 14));
        assert!(!has_digit_run("only 1234567 here", 14));
    }

    #[test]
    fn the_public_tier_is_provably_pii_free() {
        // Core invariant: NOTHING is_pii ever lands at LEVEL_PUBLIC.
        let pii_rows = [
            ("reports/legal/x.md", "a"),
            ("readme.md", "cnpj 12345678000199 here"), // public-named but PII content
            ("docs/what-is-asolaria.md", "paypal ..."),
            ("x/.asolaria/recall.key", "secret"),
        ];
        for (p, c) in pii_rows {
            assert_ne!(
                assign_level(p, c),
                LEVEL_PUBLIC,
                "PII row leaked to public: {p}"
            );
        }
    }

    #[test]
    fn coverage_gap_findings_2026_06_22_are_owner_private() {
        // Paths the adversarial audit (workflow wkrt8surs over acer's real 591,286-row index) found
        // leaking to FEDERATION(5) — and one to PUBLIC(0) — instead of owner-private. Pinned to 9.
        let must_be_private = [
            "C:/asolaria-acer/packages/immune-l1-supervisor/keys/supervisor.ed25519.pem",
            "C:/Users/acer/Asolaria/data/vault.master.key",
            "C:/Users/acer/Asolaria/data/vault/owner/crypto-capsule/ed25519.key.pem",
            "C:/Users/acer/Asolaria/sovereignty/data/falcon-dump/sdcard/Download/April bank statement.PDF",
            "C:/Users/acer/Asolaria/data/s22-legal-recovery/packet.txt",
            "C:/Users/acer/Asolaria/tools/google-password-candidates.py",
            "D:/Asolaria-RECOVERED/Asolaria/bank-account-transcrtions/README.md", // was PUBLIC(0) via 'readme'
            "C:/Users/acer/.asolaria",                                            // bare secrets dir (was FED)
            "C:/x/s22-mounted-access/dcim.json",                                  // bare dcim filename (was FED)
            "C:/asolaria-acer/packages/dashboard/.../pdf-claude-secret-settings.txt",
        ];
        for p in must_be_private {
            assert_eq!(
                assign_level(p, ""),
                LEVEL_OWNER_PRIVATE,
                "still leaking (not owner-private): {p}"
            );
        }
        // And the public-canon docs must STILL be public (the expansion must not over-privatize them).
        for p in [
            "C:/asolaria-acer/README.md",
            "C:/Users/acer/Asolaria/BROWN-HILBERT.md",
            "reports/asolaria-multi-cylinder-v2.html",
            "docs/what-is-asolaria.md",
        ] {
            assert_eq!(
                assign_level(p, "clean canon body"),
                LEVEL_PUBLIC,
                "canon wrongly privatized: {p}"
            );
        }
    }
}
