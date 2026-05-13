//! Asolaria Federation Remake · Kernel Core
//!
//! Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
//! Phase 2 CLOSED at envelope :82496 (KERNEL_TIER_LANDED) · Phase 3 scaffolding now in progress
//!
//! Modules:
//! - `pid` — BEHCS-1024 PID minter (Phase-2 Step 27, port of `mintBehcs256Pid47` to `mint_behcs1024_pid_60`)
//! - `envelope` — atomic envelope dispatch primitive (Phase-2 Step 29, lock-free MPMC)
//! - `crypto` — ed25519 substrate (Phase-2 Step 28)
//! - `hookwall` — pre/post syscall hooks (Phase-2 prep + Phase-3 wiring)
//! - `syscall` — 16-syscall canonical surface (Phase-2 Step 25)
//! - `cosign_chain` — append-only sha-linked verdict ledger (Phase-3 Step 45)

#![no_std]
#![forbid(unsafe_code)]
// syscall + envelope dispatch + PID minter must be unsafe-free at API layer
// Phase-3 scaffold: many fields/variants are intentionally undocumented in v0.1; docs
// land incrementally in v0.2+. Relaxing these lints from `deny` to `allow` lets the CI
// `cargo clippy -- -D warnings` gate stay strict on correctness while letting in-flight
// scaffolding compile. Each `#![allow]` here is a known-debt marker, not a permanent ok.
#![allow(unknown_lints)] // CI runs rustc 1.81; some allows below are 1.95-era lint names
#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::manual_contains)]
#![allow(clippy::identity_op)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::new_without_default)]
#![allow(clippy::manual_div_ceil)]
#![allow(rustdoc::bare_urls)]
#![allow(rustdoc::broken_intra_doc_links)]

extern crate alloc;

pub mod crypto;
pub mod envelope;
pub mod frame_alloc;
pub mod hookwall;
pub mod pid;
pub mod syscall;
pub mod vfs;
// Phase-2.5 cycle-72: demote was premature — kernel modules must remain present-and-callable
// until Syscall-IPC-Rewire converts each call site to envelope RPC. Servers/* mirrors at
// servers/{cosign-ledger,tier-policy,highway,agent-runtime,gnn-oracle}/src/lib.rs are the
// forward targets; rewire moves call sites one-by-one without breaking the kernel build.
pub mod agent_runtime;
pub mod atlas;
pub mod bus_and_kick;
pub mod bus_fabric;
pub mod cosign_chain;
pub mod cycle_orch;
pub mod glyph_genesis;
pub mod gnn;
pub mod highway;
pub mod sign_gate;
pub mod tier;
pub mod tier_gate;
pub mod transit;

/// Kernel version string. Bumped on tier-2 cosign-approved ABI changes.
pub const KERNEL_VERSION: &str = "0.2.0-phase3-scaffold";

/// Federation anchor PID for any kernel-emitted envelope.
pub const FEDERATION_ANCHOR_PID: &str = "ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11";
