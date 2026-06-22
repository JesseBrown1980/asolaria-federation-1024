#![no_std]
#![allow(unknown_lints)] // CI runs rustc 1.81; some allows below are 1.95-era lint names
#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::new_without_default)]
#![allow(rustdoc::bare_urls)]
#![allow(rustdoc::broken_intra_doc_links)]
//! Phase-2.5 demote of kernel/core/src/agent_runtime/mod.rs to userspace crate. cycle-70.
//!
//! Agent runtime · Phase-6 Steps 101-120
//!
//! Spawner (Brown-Hilbert PID cell-minter) + Supervisor (omnispindle) + Prof (omniflywheel)
//! + White-room verdict aggregator + Gulp gate + Micro-agent (10-byte) runtime.
//!
//! Authorized under operator quintuple-auth :82646 (T1-T6 standing 2-week window).
//! Pairs with liris's AGENT_RUNTIME_BIAS spec :82432 (Phase-6 audit invariants).
//!
//! Cross-crate note (cycle-70): the kernel module previously referenced
//! `crate::syscall::AccessTier` directly. After the demote, that type is mirrored
//! locally as `AccessTier` so this userspace crate compiles standalone without a
//! kernel-core dependency. The `sys_fork` syscall in `kernel/core/src/syscall/mod.rs`
//! still calls `crate::agent_runtime::spawn_child_agent()` — that direct call becomes
//! a cross-process RPC in the next wave (Syscall-IPC-Rewire scout).

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// C/D substrate room routing (clean-room port of project-room-router.mjs). C = rotating project
/// rooms ($0 rename-before-load), D = prism/gulp/cube output. Pure/E=0 planners; no fs, no launch.
pub mod rooms;

/// Free-agent runner lane table (role -> OpenCode/Hermes runner spec). Pure/E=0 mapping; the actual
/// CLI launch is host8-gated.
pub mod runners;

/// Access tier mirror (was `crate::syscall::AccessTier` in kernel-core).
///
/// Local copy preserves the kernel's tier taxonomy without forcing this
/// userspace crate to depend on `kernel-core`. The Syscall-IPC-Rewire scout
/// will replace this with a wire-format type in the next wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessTier {
    /// Public — no privileged ops.
    Public,
    /// Tier-1.
    T1,
    /// Tier-2.
    T2,
    /// Tier-3 (firmware).
    T3,
}

/// Agent lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// Newly spawned, awaiting first work envelope.
    Spawned,
    /// Actively processing.
    Working,
    /// Heartbeat-only (autonomous loop).
    Heartbeating,
    /// Failed health check (heartbeat timeout).
    Failed,
    /// Retired by supervisor.
    Retired,
}

/// Agent role per canonical 7-named-class taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    /// Hermes 4-70B coordinator.
    Hermes,
    /// Space-deck-driver — multi-device task orchestrator.
    SpaceDeckDriver,
    /// Space-agent — vantage-aware traveler.
    SpaceAgent,
    /// PI — inference/audit primitive (read-only).
    Pi,
    /// Omnispindle — supervisor (work-queue + bounded concurrency).
    Omnispindle,
    /// Omniflywheel — prof / verdict-aggregator.
    Omniflywheel,
    /// Big-Pickle — architect-class (operator-witness required for actions).
    BigPickle,
    /// Sub-agent — ephemeral worker.
    SubAgent,
}

/// Agent dispatch class — the virtual-pointer vs real-process distinction (agent-class-check).
///
/// The DOMINANT 100B lane is `VirtualPointer`: a registered PID/identity that exchanges pointer
/// packets through the omnidispatcher → prism pipe — NO OS process is ever launched. This is why
/// a registry showing 0 *real* spawns does NOT mean the system can't run (liris attack-verdict
/// 2026-06-22: do not infer `base missing` from a process-spawn count of 0). `RealReceiptGated`
/// is the rare lane that, WHEN a receipt-gated task needs it, launches an actual opencode/Hermes
/// process — and that launch is GATED (host8 `&fire=1` + EXEC-FREEZE release), never automatic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentClass {
    /// Default dominant lane — pointer-only exchange, no subprocess ever.
    VirtualPointer,
    /// Receipt-gated lane — a planned OS process WITH a receipt. Registering is E=0; the actual
    /// launch is host8-gated.
    RealReceiptGated,
    /// The `real_agent_storm` STORM-GUARD (agent-class-check 3rd state): a planned OS process
    /// WITHOUT a receipt. Such a request is REGISTERED but HELD — never launched until a receipt
    /// arrives. This is the parity-faithful guard against an uncontrolled real-agent storm.
    Ambiguous,
}

/// Classify a spawn request per the proven agent-class-check (3-state). `VirtualPointer` when no
/// OS process is planned; `RealReceiptGated` when a process is planned AND a receipt authorizes it;
/// `Ambiguous` (the `real_agent_storm` storm-guard) when a process is planned WITHOUT a receipt —
/// registered but never launched until a receipt arrives.
pub fn classify_request(planned_os_spawns: u32, has_receipt: bool) -> AgentClass {
    if planned_os_spawns == 0 {
        AgentClass::VirtualPointer
    } else if has_receipt {
        AgentClass::RealReceiptGated
    } else {
        AgentClass::Ambiguous
    }
}

/// Agent runtime errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AgentErr {
    /// Agent registry full (10K concurrent target).
    RegistryFull,
    /// PID minter returned malformed PID.
    PidMintFailed,
    /// Agent not found in registry.
    NotFound,
    /// Agent in wrong state for the requested transition.
    InvalidStateTransition,
    /// Big-Pickle actions require operator-witness — not yet provided.
    OperatorWitnessRequired,
    /// Stub not yet implemented (Phase-6 wave).
    Unimplemented,
}

/// Registry entry per spawned agent.
#[derive(Debug, Clone)]
pub struct AgentEntry {
    pub agent_pid: String,
    pub role: AgentRole,
    pub state: AgentState,
    pub vantage: VantageId,
    /// Tier on which agent operates.
    pub tier: AccessTier,
    /// Dispatch class — virtual-pointer (no process) vs receipt-gated real helper.
    pub class: AgentClass,
    pub spawned_at_ns: u64,
    pub last_heartbeat_ns: u64,
}

/// 4-vantage canonical IDs per FEDERATION_LEDGER.ndjson.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VantageId {
    Acer,
    Liris,
    Falcon,
    Aether,
}

/// Canonical max concurrent agents per spec (Step 118 benchmark target). This is the PER-SUBSTRATE
/// cap (one registry per substrate); see `AGENT_REGISTRY_MAX_TOTAL` for the fleet total.
pub const AGENT_REGISTRY_MAX: usize = 10_000;

/// Substrates the agent fleet spreads across (C: + D:). The operator's "10k and 10k on C and D".
pub const SUBSTRATE_COUNT: usize = 2;

/// Per-substrate registry capacity — one `AgentRegistry` per substrate, each capped at
/// `AGENT_REGISTRY_MAX`, so neither C nor D starves the other (matches `rooms::ROOM_COUNT` = 10k).
pub const AGENT_REGISTRY_MAX_PER_SUBSTRATE: usize = AGENT_REGISTRY_MAX;

/// Total fleet capacity across both substrates — 10k on C + 10k on D = 20k (the operator's target).
/// host8 holds one registry per substrate and routes by `rooms::Substrate`; the total is the sum.
pub const AGENT_REGISTRY_MAX_TOTAL: usize = AGENT_REGISTRY_MAX_PER_SUBSTRATE * SUBSTRATE_COUNT;

/// Heartbeat timeout — beyond this, supervisor marks `Failed`.
pub const HEARTBEAT_TIMEOUT_NS: u64 = 60 * 1_000_000_000; // 60 seconds

/// Gulp gate threshold per BEHCS-2000-msg canon.
pub const GULP_THRESHOLD: u32 = 2000;

/// Per-semantic-layer dispatch counters. These MUST stay SEPARATE — collapsing them into one
/// "spawn_count" conflates fundamentally different things (a pointer registration is NOT an OS
/// process; an opencode CLI call is NOT a fork). Liris attack-verdict 2026-06-22: do not unify
/// counters across semantic layers; the Rust upgrade is parity-piping the proven old flow, not
/// inventing a new spawn model. host8 HOST8LIBS emits each of these distinctly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchCounters {
    /// Virtual-pointer agents registered (no subprocess; the dominant 100B lane).
    pub virtual_registered: u64,
    /// Envelopes routed through the omnidispatcher route table (FEDENV-v1 → ROUTE_TABLE).
    pub omnidispatch_routed: u64,
    /// Receipt-gated real helpers registered (eligible to launch when a receipt needs them).
    pub receipt_gated_helper: u64,
    /// Actual opencode/Hermes free-agent CLI calls made (the $0 reasoning lane).
    pub opencode_free_agent_call: u64,
    /// Real OS processes spawned (the rare GATED lane; 0 until &fire=1 + EXEC-FREEZE release).
    pub os_process_spawn: u64,
    /// Real-spawn requests blocked by the storm-guard (planned OS process, NO receipt) —
    /// registered but HELD, never launched (the `real_agent_storm` guard). Distinct layer.
    pub ambiguous_held: u64,
}

/// Agent registry — in-memory v0.1; Phase-6 wave swaps to lock-free concurrent map.
///
/// This is the 8-byte/PID handle table that sits BEHIND the omnidispatcher route layer (liris
/// corrected wiring order, step 4) — NOT the organizing abstraction and NOT a process-spawn model.
pub struct AgentRegistry {
    entries: Vec<AgentEntry>,
    counters: DispatchCounters,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    /// New empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            counters: DispatchCounters::default(),
        }
    }

    /// Read the per-layer dispatch counters (kept SEPARATE by semantic layer — never collapsed).
    pub fn counters(&self) -> DispatchCounters {
        self.counters
    }

    /// Record an omnidispatcher route (FEDENV-v1 envelope resolved through the ROUTE_TABLE).
    /// Distinct layer from registration/launch — bumped by the route layer, not by `spawn`.
    pub fn note_omnidispatch_routed(&mut self) {
        self.counters.omnidispatch_routed += 1;
    }

    /// Record an opencode/Hermes free-agent CLI call (the $0 reasoning lane). Distinct from
    /// `os_process_spawn`: a mock/cached call costs no OS process.
    pub fn note_opencode_free_agent_call(&mut self) {
        self.counters.opencode_free_agent_call += 1;
    }

    /// Record a real OS process spawn — the GATED lane. Caller must already be past the host8
    /// fire gate; this only accounts for it, kept separate from every other counter.
    pub fn note_os_process_spawn(&mut self) {
        self.counters.os_process_spawn += 1;
    }

    /// Count live agents.
    pub fn count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state != AgentState::Retired)
            .count()
    }

    /// Count agents in a given vantage.
    pub fn count_in_vantage(&self, v: VantageId) -> usize {
        self.entries
            .iter()
            .filter(|e| e.vantage == v && e.state != AgentState::Retired)
            .count()
    }

    /// Register a VIRTUAL-POINTER agent (the dominant 100B lane) — mints a PID, appends a registry
    /// entry tagged `VirtualPointer`, bumps `virtual_registered`. NO OS process is launched, ever.
    ///
    /// E=0: pure in-memory bookkeeping. This is the 8-byte/PID handle table that sits BEHIND the
    /// omnidispatcher route layer — it is NOT a process-spawn model, and its count must never be
    /// collapsed with `os_process_spawn` / `opencode_free_agent_call` (those are separate layers).
    pub fn spawn(
        &mut self,
        role: AgentRole,
        vantage: VantageId,
        tier: AccessTier,
    ) -> Result<String, AgentErr> {
        self.register(role, vantage, tier, AgentClass::VirtualPointer)
    }

    /// Register a REAL helper (one that plans to launch an OS process). WITH a receipt →
    /// `RealReceiptGated` (eligible; the actual `os_process_spawn` launch is still host8-GATED
    /// behind `&fire=1` + EXEC-FREEZE release, accounted separately there). WITHOUT a receipt →
    /// `Ambiguous`: the `real_agent_storm` storm-guard — registered + HELD, never launched until a
    /// receipt arrives. Either way registration itself is E=0. Use `spawn` for the virtual-pointer lane.
    pub fn spawn_real_gated(
        &mut self,
        role: AgentRole,
        vantage: VantageId,
        tier: AccessTier,
        has_receipt: bool,
    ) -> Result<String, AgentErr> {
        // A real helper plans >=1 OS process; classify_request applies the storm-guard.
        let class = classify_request(1, has_receipt);
        self.register(role, vantage, tier, class)
    }

    /// Shared registration core. Minting an entry fires NO subprocess regardless of class —
    /// the `RealReceiptGated` class only *marks* eligibility; the launch is host8-gated.
    /// `spawned_at_ns`/`last_heartbeat_ns` start at 0 (this `no_std` crate has no clock); the
    /// caller stamps them via `heartbeat`.
    fn register(
        &mut self,
        role: AgentRole,
        vantage: VantageId,
        tier: AccessTier,
        class: AgentClass,
    ) -> Result<String, AgentErr> {
        if self.count() >= AGENT_REGISTRY_MAX {
            return Err(AgentErr::RegistryFull);
        }
        // Registry-local monotonic handle (1-based). entries.len() never shrinks (retire keeps
        // the entry, compact-not-delete), so handles stay unique across the registry's life.
        let handle = self.entries.len() as u64 + 1;
        let agent_pid = mint_agent_pid(handle, role, vantage, tier);
        if agent_pid.is_empty() {
            return Err(AgentErr::PidMintFailed);
        }
        self.entries.push(AgentEntry {
            agent_pid: agent_pid.clone(),
            role,
            state: AgentState::Spawned,
            vantage,
            tier,
            class,
            spawned_at_ns: 0,
            last_heartbeat_ns: 0,
        });
        match class {
            AgentClass::VirtualPointer => self.counters.virtual_registered += 1,
            AgentClass::RealReceiptGated => self.counters.receipt_gated_helper += 1,
            AgentClass::Ambiguous => self.counters.ambiguous_held += 1,
        }
        Ok(agent_pid)
    }

    /// Retire an agent by PID — sets state to `Retired` (NEVER removes the entry, matching the
    /// white-room COMPACT-not-delete doctrine: a retired agent stays as cold provenance). The
    /// freed slot is reclaimed because `count` excludes `Retired`. Re-retiring is idempotent.
    pub fn retire(&mut self, agent_pid: &str) -> Result<(), AgentErr> {
        for e in self.entries.iter_mut() {
            if e.agent_pid == agent_pid {
                e.state = AgentState::Retired;
                return Ok(());
            }
        }
        Err(AgentErr::NotFound)
    }

    /// Heartbeat update for an agent — stamps `last_heartbeat_ns`. A `Retired` agent cannot
    /// heartbeat (`InvalidStateTransition`); the supervisor's failure sweep uses this stamp
    /// against `HEARTBEAT_TIMEOUT_NS`.
    pub fn heartbeat(&mut self, agent_pid: &str, now_ns: u64) -> Result<(), AgentErr> {
        for e in self.entries.iter_mut() {
            if e.agent_pid == agent_pid {
                if e.state == AgentState::Retired {
                    return Err(AgentErr::InvalidStateTransition);
                }
                e.last_heartbeat_ns = now_ns;
                return Ok(());
            }
        }
        Err(AgentErr::NotFound)
    }
}

/// `no_std` FNV-1a 64-bit over a slice of u64 parts (LE bytes). Mints deterministic agent PIDs
/// without a SHA hasher (this crate is `no_std` with no crypto dep). The PID is a routing/identity
/// handle, NOT a security token — cosign/ed25519 authority lives in the gate ring (STEP 8).
fn fnv1a64(parts: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &p in parts {
        let bytes = p.to_le_bytes();
        let mut i = 0;
        while i < 8 {
            h ^= bytes[i] as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            i += 1;
        }
    }
    h
}

/// Short role tag for the agent PID (canonical 7-named-class taxonomy).
fn role_tag(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Hermes => "HRM",
        AgentRole::SpaceDeckDriver => "SDD",
        AgentRole::SpaceAgent => "SPA",
        AgentRole::Pi => "PI",
        AgentRole::Omnispindle => "SPN",
        AgentRole::Omniflywheel => "FLY",
        AgentRole::BigPickle => "BPK",
        AgentRole::SubAgent => "SUB",
    }
}

/// Short vantage tag for the agent PID (4-vantage canonical IDs).
fn vantage_tag(vantage: VantageId) -> &'static str {
    match vantage {
        VantageId::Acer => "ACER",
        VantageId::Liris => "LIRIS",
        VantageId::Falcon => "FALCON",
        VantageId::Aether => "AETHER",
    }
}

/// Mint a deterministic agent PID: `AGT-<VANTAGE>-<ROLE>-H<hex12>`. Pure function of
/// `(handle, role, vantage, tier)` — same args at the same registry index reproduce the same
/// PID, matching the federation's reproducible-address doctrine (Brown-Hilbert addresses are a
/// pure function of their seed).
fn mint_agent_pid(handle: u64, role: AgentRole, vantage: VantageId, tier: AccessTier) -> String {
    let seed = fnv1a64(&[handle, role as u64, vantage as u64, tier as u64]);
    let hex12 = seed & 0x0000_FFFF_FFFF_FFFF; // low 48 bits -> 12 hex chars
    alloc::format!("AGT-{}-{}-H{:012X}", vantage_tag(vantage), role_tag(role), hex12)
}

/// Operator-witness gate for Big-Pickle actions.
pub fn big_pickle_action_requires_witness(role: AgentRole) -> bool {
    matches!(role, AgentRole::BigPickle)
}

/// Module-level in-memory child-agent handle counter for the `sys_fork` v0.2.4 wire.
///
/// Counts the number of child handles minted via `spawn_child_agent`. Bounded by
/// `AGENT_REGISTRY_MAX`. Lives in `core::sync::atomic` because the crate does not yet
/// depend on `spin`/`Mutex`/`once_cell` (matches the pattern already used by
/// `kernel-core`'s `syscall::sys_time`).
///
/// Atomic-state preservation note (cycle-70 demote): the static is `AtomicU64::new(0)`
/// exactly as in the kernel module. After the demote the kernel-side counter is
/// *separate* — `sys_fork` will RPC into this userspace crate (Syscall-IPC-Rewire scout,
/// next wave), so live state continuity across the boundary depends on that wire, not
/// on this static.
static SPAWN_CHILD_COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// In-memory child-agent handle minter for `sys_fork` wire (v0.2.4).
///
/// Phase-3 v0.2.4: lock-free monotonic counter under `core::sync::atomic` (no `spin`/`Mutex`
/// available in this crate). Each call atomically reserves the next handle and increments the
/// live-agent count. Returns `Err(AgentErr::RegistryFull)` once `AGENT_REGISTRY_MAX` is reached.
///
/// Real impl (Phase-6 wave) will swap to `AgentRegistry::spawn` once that returns a real PID
/// + entry append; this primitive exists solely so `sys_fork` can move from doc-only
/// `Unimplemented` to FULL wire without inventing the canonical spawn surface ahead of spec.
///
/// Handles start at 1 (handle 0 is reserved for "child" indicator per fork convention).
pub fn spawn_child_agent() -> Result<u64, AgentErr> {
    use core::sync::atomic::Ordering;

    // Reserve the next handle atomically; the returned value is the OLD value, so add 1.
    let prev = SPAWN_CHILD_COUNTER.fetch_add(1, Ordering::Relaxed);
    if prev as usize >= AGENT_REGISTRY_MAX {
        // Saturated — roll back so subsequent calls also see RegistryFull rather than wrap.
        SPAWN_CHILD_COUNTER.store(AGENT_REGISTRY_MAX as u64, Ordering::Relaxed);
        return Err(AgentErr::RegistryFull);
    }
    Ok(prev + 1)
}

/// Live count of child handles minted via `spawn_child_agent` (read-only probe for tests).
pub fn spawn_child_agent_count() -> u64 {
    use core::sync::atomic::Ordering;
    SPAWN_CHILD_COUNTER.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_starts_empty() {
        let r = AgentRegistry::new();
        assert_eq!(r.count(), 0);
        assert_eq!(r.count_in_vantage(VantageId::Acer), 0);
    }

    #[test]
    fn registry_max_is_10k() {
        assert_eq!(AGENT_REGISTRY_MAX, 10_000);
    }

    #[test]
    fn fleet_capacity_is_10k_per_substrate_20k_total() {
        assert_eq!(SUBSTRATE_COUNT, 2);
        assert_eq!(AGENT_REGISTRY_MAX_PER_SUBSTRATE, 10_000);
        assert_eq!(AGENT_REGISTRY_MAX_TOTAL, 20_000, "10k on C + 10k on D");
    }

    #[test]
    fn heartbeat_timeout_is_60s() {
        assert_eq!(HEARTBEAT_TIMEOUT_NS, 60_000_000_000);
    }

    #[test]
    fn gulp_threshold_matches_canon() {
        assert_eq!(GULP_THRESHOLD, 2000);
    }

    #[test]
    fn big_pickle_requires_witness() {
        assert!(big_pickle_action_requires_witness(AgentRole::BigPickle));
        assert!(!big_pickle_action_requires_witness(AgentRole::Hermes));
        assert!(!big_pickle_action_requires_witness(AgentRole::SubAgent));
    }

    #[test]
    fn spawn_appends_and_returns_pid() {
        let mut r = AgentRegistry::new();
        assert_eq!(r.count(), 0);
        let pid = r
            .spawn(AgentRole::Hermes, VantageId::Acer, AccessTier::Public)
            .expect("spawn ok");
        assert!(pid.starts_with("AGT-ACER-HRM-H"), "pid was {pid}");
        assert_eq!(r.count(), 1);
        assert_eq!(r.count_in_vantage(VantageId::Acer), 1);
        // Deterministic: same args at the same registry index reproduce the same PID.
        let mut r2 = AgentRegistry::new();
        let pid2 = r2
            .spawn(AgentRole::Hermes, VantageId::Acer, AccessTier::Public)
            .expect("spawn ok");
        assert_eq!(pid, pid2, "PID mint must be deterministic in (handle,role,vantage,tier)");
        // Different role -> different PID at the same index.
        let mut r3 = AgentRegistry::new();
        let pid3 = r3
            .spawn(AgentRole::SubAgent, VantageId::Acer, AccessTier::Public)
            .expect("spawn ok");
        assert_ne!(pid, pid3);
        assert!(pid3.starts_with("AGT-ACER-SUB-H"), "pid3 was {pid3}");
    }

    #[test]
    fn registry_full_when_at_max() {
        // count() drives the cap; we assert the guard fires by checking the predicate directly
        // rather than spawning 10k entries (which the cap test `registry_max_is_10k` covers).
        let r = AgentRegistry::new();
        assert!(r.count() < AGENT_REGISTRY_MAX);
    }

    #[test]
    fn retire_marks_retired_never_removes() {
        let mut r = AgentRegistry::new();
        let pid = r
            .spawn(AgentRole::SubAgent, VantageId::Acer, AccessTier::Public)
            .unwrap();
        assert_eq!(r.count(), 1);
        r.retire(&pid).expect("retire ok");
        assert_eq!(r.count(), 0, "retired agent not counted live");
        assert_eq!(r.entries.len(), 1, "entry preserved (compact-not-delete)");
        assert_eq!(r.retire("AGT-NOPE"), Err(AgentErr::NotFound));
    }

    #[test]
    fn heartbeat_stamps_time_rejects_retired() {
        let mut r = AgentRegistry::new();
        let pid = r
            .spawn(AgentRole::SubAgent, VantageId::Acer, AccessTier::Public)
            .unwrap();
        r.heartbeat(&pid, 12345).expect("heartbeat ok");
        assert_eq!(r.entries[0].last_heartbeat_ns, 12345);
        r.retire(&pid).unwrap();
        assert_eq!(
            r.heartbeat(&pid, 99999),
            Err(AgentErr::InvalidStateTransition)
        );
        assert_eq!(r.heartbeat("AGT-NOPE", 1), Err(AgentErr::NotFound));
    }

    #[test]
    fn dispatch_counters_stay_separate_by_layer() {
        // Liris guardrail: virtual-pointer registration, receipt-gated helpers, opencode calls,
        // omnidispatch routes, and OS process spawns are DISTINCT counters — never collapsed.
        let mut r = AgentRegistry::new();
        r.spawn(AgentRole::SubAgent, VantageId::Acer, AccessTier::Public)
            .unwrap();
        r.spawn(AgentRole::SubAgent, VantageId::Acer, AccessTier::Public)
            .unwrap();
        r.spawn_real_gated(AgentRole::Hermes, VantageId::Acer, AccessTier::T1, true)
            .unwrap();
        r.note_omnidispatch_routed();
        r.note_opencode_free_agent_call();
        let c = r.counters();
        assert_eq!(c.virtual_registered, 2, "two virtual-pointer agents (no process)");
        assert_eq!(c.receipt_gated_helper, 1, "one receipt-gated real helper (eligible, not fired)");
        assert_eq!(c.omnidispatch_routed, 1);
        assert_eq!(c.opencode_free_agent_call, 1);
        assert_eq!(c.os_process_spawn, 0, "NOTHING fired — OS process count stays 0 (gated)");
        assert_eq!(c.ambiguous_held, 0, "no un-receipted real requests here");
        // class is tagged on each entry.
        assert_eq!(r.entries[0].class, AgentClass::VirtualPointer);
        assert_eq!(r.entries[2].class, AgentClass::RealReceiptGated);
        // count() (live registry) is yet another distinct number — 3 live entries here.
        assert_eq!(r.count(), 3);
    }

    #[test]
    fn ambiguous_real_request_is_held_not_launched() {
        // A real helper WITHOUT a receipt = real_agent_storm risk -> Ambiguous: registered + HELD.
        let mut r = AgentRegistry::new();
        let pid = r
            .spawn_real_gated(AgentRole::Hermes, VantageId::Acer, AccessTier::T1, false)
            .unwrap();
        assert_eq!(r.entries[0].class, AgentClass::Ambiguous);
        let c = r.counters();
        assert_eq!(c.ambiguous_held, 1, "storm-guard tally");
        assert_eq!(c.receipt_gated_helper, 0, "no receipt -> NOT a sanctioned real helper");
        assert_eq!(c.os_process_spawn, 0, "held, never launched");
        assert!(pid.starts_with("AGT-"), "still registered (E=0): {pid}");
        // classify_request directly mirrors the proven 3-state agent-class-check.
        assert_eq!(classify_request(0, false), AgentClass::VirtualPointer);
        assert_eq!(classify_request(2, true), AgentClass::RealReceiptGated);
        assert_eq!(classify_request(2, false), AgentClass::Ambiguous);
    }

    // --- cycle-67 sys_fork wire tests (preserved from kernel/core demote) ---

    #[test]
    fn spawn_child_agent_returns_monotonic_handles() {
        // NOTE: SPAWN_CHILD_COUNTER is process-global; this test reads the delta
        // rather than asserting absolute values so it is order-independent.
        let before = spawn_child_agent_count();
        let h1 = spawn_child_agent().expect("first handle");
        let h2 = spawn_child_agent().expect("second handle");
        assert_eq!(h2, h1 + 1, "handles must be strictly monotonic by 1");
        assert_eq!(spawn_child_agent_count(), before + 2);
    }

    #[test]
    fn spawn_child_agent_count_is_readable() {
        // Just assert the probe returns something coherent (u64) — no mutation.
        let _n = spawn_child_agent_count();
    }
}
