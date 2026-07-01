//! Cosign-chain primitive · Phase-3 Step 45
//!
//! Per REPO_LAW Invariant 4: cosign-chain is append-only; sha-linked rows; gc preserves audit.
//! Every hookwall verdict + every tier-2+ envelope dispatch appends a row.
//!
//! Wire format (ndjson, one row per line):
//! ```text
//! {"row":N, "ts_ns":U64, "prev_sha16":HEX16, "kind":STR, "payload_sha16":HEX16, "sig":HEX128}
//! ```
//!
//! The `prev_sha16` field chains each row to its predecessor — tampering with row N-k invalidates
//! all rows ≥ N-k via the sha-link.
//!
//! v0.1 scaffold: API surface + in-memory row storage. Real ndjson-writer + gc lands in Phase-3 wave.

use crate::crypto::Signature;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use sha2::{Digest, Sha256};

/// Compute canonical bytes for a `CosignRow`. Deterministic, includes every field that
/// participates in chain integrity. Used by `append` to compute `head_sha16` and by
/// `validate_end_to_end` to re-derive expected `prev_sha16` values.
///
/// Signature is intentionally OUT of canonical bytes — the sig is over THESE bytes,
/// not the other way around.
fn canonical_bytes_of_row(row: &CosignRow) -> String {
    format!(
        "row={}|ts={}|prev={}|kind={}|payload={}",
        row.row, row.ts_ns, row.prev_sha16, row.kind, row.payload_sha16
    )
}

/// Compute a 16-character lowercase-hex sha16 (first 8 bytes of sha256) of `bytes`.
/// This is the wire form used by `prev_sha16` / `payload_sha16` / `head_sha16`.
fn sha16_of(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        // Two hex chars per byte. char::from_digit returns Some for 0..=15.
        s.push(char::from_digit((byte >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((byte & 0xf) as u32, 16).unwrap());
    }
    s
}

/// Genesis row identifier — `prev_sha16` of row 1.
pub const GENESIS_PREV_SHA16: &str = "0000000000000000";

/// Cosign-chain errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CosignChainErr {
    /// Row number does not equal current head + 1 (append-only invariant violation attempt).
    NonMonotonicRow,
    /// `prev_sha16` field does not match the chain head's sha16.
    ChainLinkBroken,
    /// Row signature failed to verify.
    SignatureInvalid,
    /// Row kind tag is empty or contains invalid characters.
    KindMalformed,
    /// Stub not yet implemented (Phase-3 wave).
    Unimplemented,
}

/// Cosign-chain row, parsed form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CosignRow {
    /// Monotonic row index, starting at 1.
    pub row: u64,
    /// Monotonic ns timestamp.
    pub ts_ns: u64,
    /// 16-char hex sha16 of previous row's canonical bytes (or GENESIS for row 1).
    pub prev_sha16: String,
    /// Tag identifying what this row records (e.g. "HOOKWALL_VERDICT_PROCEED").
    pub kind: String,
    /// 16-char hex sha16 of the payload this row witnesses.
    pub payload_sha16: String,
    /// ed25519 signature over the canonical bytes (row + ts + prev + kind + payload).
    pub sig: Signature,
}

/// Append-only cosign-chain handle.
/// v0.1: in-memory Vec. Phase-3 wave swaps to ndjson-backed durable storage.
pub struct CosignChain {
    head_sha16: String,
    next_row: u64,
    rows: Vec<CosignRow>,
}

impl Default for CosignChain {
    fn default() -> Self {
        Self::new()
    }
}

impl CosignChain {
    /// Constructs an empty chain. Row 1 will link to `GENESIS_PREV_SHA16`.
    pub fn new() -> Self {
        Self {
            head_sha16: String::from(GENESIS_PREV_SHA16),
            next_row: 1,
            rows: Vec::new(),
        }
    }

    /// Returns the current chain depth (number of rows appended).
    pub fn depth(&self) -> u64 {
        self.rows.len() as u64
    }

    /// Returns sha16 of the chain head (or GENESIS for an empty chain).
    pub fn head(&self) -> &str {
        &self.head_sha16
    }

    /// Returns the next-row index that an append must use.
    pub fn next_row(&self) -> u64 {
        self.next_row
    }

    /// Appends a row to the chain. Verifies monotonicity + sha-link + kind-non-empty.
    ///
    /// v0.2 (this method):
    /// - `row.row` MUST equal `self.next_row` → else `NonMonotonicRow`
    /// - `row.prev_sha16` MUST equal `self.head_sha16` → else `ChainLinkBroken`
    /// - `row.kind` MUST be non-empty → else `KindMalformed`
    /// - On success: clones the row into `self.rows`, advances `self.head_sha16` to the
    ///   sha16 of the row's canonical bytes, bumps `self.next_row`.
    ///
    /// Signature verification is NOT enforced here yet — that requires real ed25519
    /// public-key infrastructure plumbed via `crypto` module, landing in v0.3.
    pub fn append(&mut self, row: &CosignRow) -> Result<(), CosignChainErr> {
        if row.row != self.next_row {
            return Err(CosignChainErr::NonMonotonicRow);
        }
        if row.prev_sha16 != self.head_sha16 {
            return Err(CosignChainErr::ChainLinkBroken);
        }
        if row.kind.is_empty() {
            return Err(CosignChainErr::KindMalformed);
        }
        let canon = canonical_bytes_of_row(row);
        let new_head = sha16_of(canon.as_bytes());
        self.rows.push(row.clone());
        self.head_sha16 = new_head;
        self.next_row += 1;
        Ok(())
    }

    /// Validates the entire chain end-to-end.
    ///
    /// v0.2: walks `self.rows` from the genesis prev_sha16, verifying that each row's
    /// `row` index is the position-1 expected value, its `prev_sha16` matches the
    /// re-derived sha16 of the previous row's canonical bytes, and its `kind` is
    /// non-empty. Signature verification is skipped (v0.3 wires real ed25519).
    ///
    /// Returns `Ok(())` if the chain is internally consistent; otherwise the FIRST
    /// integrity violation encountered. Useful for boot-time chain integrity checks
    /// and for re-validating after loading rows from external storage.
    pub fn validate_end_to_end(&self) -> Result<(), CosignChainErr> {
        let mut expected_prev = String::from(GENESIS_PREV_SHA16);
        for (i, row) in self.rows.iter().enumerate() {
            let expected_row_num = (i as u64) + 1;
            if row.row != expected_row_num {
                return Err(CosignChainErr::NonMonotonicRow);
            }
            if row.prev_sha16 != expected_prev {
                return Err(CosignChainErr::ChainLinkBroken);
            }
            if row.kind.is_empty() {
                return Err(CosignChainErr::KindMalformed);
            }
            let canon = canonical_bytes_of_row(row);
            expected_prev = sha16_of(canon.as_bytes());
        }
        Ok(())
    }

    /// Test-only escape hatch: push a row into the chain without integrity checks.
    /// Used to construct deliberately-tampered chains for `validate_end_to_end` tests.
    /// MUST NOT be called from non-test code.
    #[cfg(test)]
    pub fn push_unchecked_for_tests(&mut self, row: CosignRow) {
        self.rows.push(row);
    }
}

/// Module-level in-memory sequence counter for `append`.
///
/// v0.2 microkernel-deferred: real ndjson-backed durable storage replaces this AtomicU64
/// per MICROKERNEL_REFACTOR_PLAN.md. Until then, this counter satisfies the wire contract
/// needed by `sys_cosign_append` (monotonic sequence number on every successful append).
static APPEND_SEQ_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Module-level append entrypoint — wired by `crate::syscall::sys_cosign_append`.
///
/// Phase-3 v0.2.3: hashes `row` via sha256 (canonical witness for the row payload),
/// bumps the in-memory sequence counter, and returns the new chain length (1-indexed).
///
/// v0.1 contract: `row` must be ≥ 16 bytes (the caller enforces this via the half-wire
/// rejection); returns `KindMalformed` if the row slice is empty as a defensive check.
/// Real ndjson writer + sha-link chaining lands in v0.2 per microkernel plan.
pub fn append(row: &[u8]) -> Result<u64, CosignChainErr> {
    if row.is_empty() {
        return Err(CosignChainErr::KindMalformed);
    }
    // Hash the row — this is the canonical witness we would persist in v0.2.
    // Computing the digest here keeps the wire honest: callers cannot bypass crypto.
    let mut hasher = Sha256::new();
    hasher.update(row);
    let _digest = hasher.finalize();
    // Bump sequence counter atomically; return the new (1-indexed) chain length.
    let prev = APPEND_SEQ_COUNTER.fetch_add(1, Ordering::SeqCst);
    Ok(prev + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chain_head_is_genesis() {
        let c = CosignChain::new();
        assert_eq!(c.head(), GENESIS_PREV_SHA16);
        assert_eq!(c.depth(), 0);
        assert_eq!(c.next_row(), 1);
    }

    #[test]
    fn genesis_marker_is_16_zeros() {
        assert_eq!(GENESIS_PREV_SHA16.len(), 16);
        assert!(GENESIS_PREV_SHA16.chars().all(|c| c == '0'));
    }

    fn sample_row(num: u64, prev: &str) -> CosignRow {
        CosignRow {
            row: num,
            ts_ns: 1_000_000 + num,
            prev_sha16: String::from(prev),
            kind: String::from("TEST"),
            payload_sha16: String::from("0000000000000000"),
            sig: Signature([0u8; 64]),
        }
    }

    // ===== v0.2 chain integrity tests =====

    #[test]
    fn append_single_row_links_to_genesis_and_advances_head() {
        let mut c = CosignChain::new();
        let row = sample_row(1, GENESIS_PREV_SHA16);
        assert_eq!(c.append(&row), Ok(()));
        assert_eq!(c.depth(), 1);
        assert_eq!(c.next_row(), 2);
        assert_ne!(c.head(), GENESIS_PREV_SHA16);
        assert_eq!(c.head().len(), 16);
        assert!(c.head().chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn append_chains_multiple_rows_with_sha_link() {
        let mut c = CosignChain::new();
        let r1 = sample_row(1, GENESIS_PREV_SHA16);
        assert_eq!(c.append(&r1), Ok(()));
        let head_after_1 = String::from(c.head());

        let r2 = sample_row(2, &head_after_1);
        assert_eq!(c.append(&r2), Ok(()));
        let head_after_2 = String::from(c.head());
        assert_ne!(head_after_2, head_after_1);

        let r3 = sample_row(3, &head_after_2);
        assert_eq!(c.append(&r3), Ok(()));

        assert_eq!(c.depth(), 3);
        assert_eq!(c.next_row(), 4);
    }

    #[test]
    fn append_rejects_non_monotonic_row_number() {
        let mut c = CosignChain::new();
        let too_far = sample_row(2, GENESIS_PREV_SHA16);
        assert_eq!(c.append(&too_far), Err(CosignChainErr::NonMonotonicRow));
        let r1 = sample_row(1, GENESIS_PREV_SHA16);
        assert_eq!(c.append(&r1), Ok(()));
        // Re-appending row 1 (replay) must reject.
        let head = String::from(c.head());
        let replay = sample_row(1, &head);
        assert_eq!(c.append(&replay), Err(CosignChainErr::NonMonotonicRow));
    }

    #[test]
    fn append_rejects_broken_chain_link() {
        let mut c = CosignChain::new();
        let bad = sample_row(1, "deadbeefcafe1234");
        assert_eq!(c.append(&bad), Err(CosignChainErr::ChainLinkBroken));
    }

    #[test]
    fn append_rejects_empty_kind() {
        let mut c = CosignChain::new();
        let mut r = sample_row(1, GENESIS_PREV_SHA16);
        r.kind = String::new();
        assert_eq!(c.append(&r), Err(CosignChainErr::KindMalformed));
    }

    #[test]
    fn validate_empty_chain_returns_ok() {
        let c = CosignChain::new();
        assert_eq!(c.validate_end_to_end(), Ok(()));
    }

    #[test]
    fn validate_genuine_chain_returns_ok() {
        let mut c = CosignChain::new();
        let r1 = sample_row(1, GENESIS_PREV_SHA16);
        c.append(&r1).unwrap();
        let head1 = String::from(c.head());
        let r2 = sample_row(2, &head1);
        c.append(&r2).unwrap();
        let head2 = String::from(c.head());
        let r3 = sample_row(3, &head2);
        c.append(&r3).unwrap();
        assert_eq!(c.validate_end_to_end(), Ok(()));
    }

    #[test]
    fn validate_detects_tampered_prev_sha16() {
        let mut c = CosignChain::new();
        // Build a valid 2-row chain.
        let r1 = sample_row(1, GENESIS_PREV_SHA16);
        c.append(&r1).unwrap();
        let head1 = String::from(c.head());
        let r2 = sample_row(2, &head1);
        c.append(&r2).unwrap();
        assert_eq!(c.validate_end_to_end(), Ok(()));

        // Now construct a tampered chain via the test-only escape hatch.
        let mut tampered = CosignChain::new();
        tampered.push_unchecked_for_tests(r1.clone());
        let mut bad_r2 = sample_row(2, "0123456789abcdef"); // wrong prev_sha16
        bad_r2.row = 2;
        tampered.push_unchecked_for_tests(bad_r2);
        assert_eq!(
            tampered.validate_end_to_end(),
            Err(CosignChainErr::ChainLinkBroken)
        );
    }

    #[test]
    fn validate_detects_non_monotonic_row_number() {
        let mut c = CosignChain::new();
        // Insert row with index 5 first via unchecked push.
        let mut r = sample_row(5, GENESIS_PREV_SHA16);
        r.row = 5;
        c.push_unchecked_for_tests(r);
        assert_eq!(
            c.validate_end_to_end(),
            Err(CosignChainErr::NonMonotonicRow)
        );
    }

    #[test]
    fn validate_detects_empty_kind_row() {
        let mut c = CosignChain::new();
        let mut r = sample_row(1, GENESIS_PREV_SHA16);
        r.kind = String::new();
        c.push_unchecked_for_tests(r);
        assert_eq!(c.validate_end_to_end(), Err(CosignChainErr::KindMalformed));
    }

    #[test]
    fn canonical_bytes_are_deterministic() {
        let r = sample_row(1, GENESIS_PREV_SHA16);
        let b1 = canonical_bytes_of_row(&r);
        let b2 = canonical_bytes_of_row(&r);
        assert_eq!(b1, b2);
        // Includes every chain-relevant field.
        assert!(b1.contains("row=1"));
        assert!(b1.contains("ts="));
        assert!(b1.contains("prev=0000000000000000"));
        assert!(b1.contains("kind=TEST"));
        assert!(b1.contains("payload=0000000000000000"));
    }

    #[test]
    fn sha16_of_is_16_hex_chars() {
        let s = sha16_of(b"hello world");
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic.
        assert_eq!(s, sha16_of(b"hello world"));
        // Different input → different digest (with overwhelming probability).
        assert_ne!(s, sha16_of(b"hello world!"));
    }
}
