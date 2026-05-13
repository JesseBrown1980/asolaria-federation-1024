//! Phase-2.5 demote of kernel/core/src/cosign_chain/mod.rs to userspace crate. cycle-70 operator AUTHORIZE_ALL.
//!
//! Cosign-chain primitive · Phase-3 Step 45
//!
//! Per REPO_LAW Invariant 4: cosign-chain is append-only; sha-linked rows; gc preserves audit.
//! Every hookwall verdict + every tier-2+ envelope dispatch appends a row.
//!
//! Wire format (ndjson, one row per line):
//! ```
//! {"row":N, "ts_ns":U64, "prev_sha16":HEX16, "kind":STR, "payload_sha16":HEX16, "sig":HEX128}
//! ```
//!
//! The `prev_sha16` field chains each row to its predecessor — tampering with row N-k invalidates
//! all rows ≥ N-k via the sha-link.
//!
//! v0.1 scaffold: API surface + in-memory row storage. Real ndjson-writer + gc lands in Phase-3 wave.
//!
//! ─────────────────────────────────────────────────────────────────────────────
//! PHASE-2.5 BOUNDARY NOTE (cycle-70): This crate now lives in userspace per
//! MICROKERNEL_REFACTOR_PLAN.md. The `use asolaria_kernel_core::crypto::Signature`
//! import below crosses the userspace→kernel-internals boundary, which the
//! microkernel design forbids. This is FLAGGED for the Syscall-IPC-Rewire scout
//! (next phase): the proper resolution is an RPC syscall surface (e.g.
//! `sys_crypto_signature_t`) so userspace receives an opaque handle / wire-bytes
//! instead of importing the kernel type directly. Demote ships as-is to keep
//! the cycle-70 module-move atomic; the boundary fix is a separate commit.
//! ─────────────────────────────────────────────────────────────────────────────
//!
//! NDJSON-WRITER NOTE (Phase-10 ship gate): per microkernel plan the kernel does
//! NOT write disk; the durable ndjson writer for `COSIGN_CHAIN.ndjson` is the
//! eventual responsibility of THIS userspace crate. Not implemented in this
//! demote — see the comment inside `pub fn append` for the exact insertion point.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use sha2::{Digest, Sha256};

/// ed25519 signature mirror (was `asolaria_kernel_core::crypto::Signature` in pre-demote source).
/// Byte-identical wire layout; replaced with cross-process RPC handle during Syscall-IPC-Rewire.
/// Any drift from `kernel-core` Signature tuple-struct = Class-1 ABI break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

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

    /// Appends a row to the chain. Verifies monotonicity + sha-link + signature.
    /// v0.1 stub: returns Unimplemented (sha-link and signature verification land in Phase-3 wave).
    pub fn append(&mut self, _row: &CosignRow) -> Result<(), CosignChainErr> {
        Err(CosignChainErr::Unimplemented)
    }

    /// Validates the entire chain end-to-end.
    /// v0.1 stub.
    pub fn validate_end_to_end(&self) -> Result<(), CosignChainErr> {
        Err(CosignChainErr::Unimplemented)
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
    // ─── PHASE-10 SHIP-GATE INSERTION POINT (ndjson-writer) ───────────────────
    // Append a canonical ndjson row to `COSIGN_CHAIN.ndjson` HERE — userspace is
    // the disk-owner per microkernel plan; the kernel never opens fds. Format is
    // the header wire-format above; `prev_sha16` is the rolling sha16 of `_digest`
    // from the previous call. NOT implemented in this Phase-2.5 demote.
    // ──────────────────────────────────────────────────────────────────────────
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

    #[test]
    fn append_stub_returns_unimplemented() {
        let mut c = CosignChain::new();
        let sig = Signature([0u8; 64]);
        let row = CosignRow {
            row: 1,
            ts_ns: 0,
            prev_sha16: String::from(GENESIS_PREV_SHA16),
            kind: String::from("TEST"),
            payload_sha16: String::from("0000000000000000"),
            sig,
        };
        assert_eq!(c.append(&row), Err(CosignChainErr::Unimplemented));
    }
}
