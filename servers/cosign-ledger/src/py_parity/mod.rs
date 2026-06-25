//! Python-daemon cosign row parity helpers.
//!
//! This module is deliberately pure `no_std` + `alloc`: it parses and canonicalizes
//! daemon JSON rows, computes the daemon sha16, and computes shadow append rows.
//! File/socket/mutex ownership stays in the `cosign-serve` std binary.

mod canon;
mod chain;
mod json_tok;

pub use canon::{canonical_bytes, persisted_line, py_escape_string, sha16};
pub use chain::{compute_append, fold_seq_max, AppendOut, ResumeState};
pub use json_tok::{parse_line, PyChainErr, PyRow, RawJson};

pub const GENESIS_PREV: &str = "0000000000000000";
pub const APPENDED_BY_DAEMON: &str = "ASOLARIA-COSIGN-CHAIN-SINGLE-WRITER-DAEMON-PID-2026-05-23";
pub const LAW_ANCHOR: &str = "ASOLARIA-FOUNDATION-V3-LAW-V39-LOAD-DIVISION-PID-2026-05-23";
