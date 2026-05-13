//! Agent runtime · Phase-6 Steps 101-120
//!
//! Spawner (Brown-Hilbert PID cell-minter) + Supervisor (omnispindle) + Prof (omniflywheel)
//! + White-room verdict aggregator + Gulp gate + Micro-agent (10-byte) runtime.
//!
//! Authorized under operator quintuple-auth :82646 (T1-T6 standing 2-week window).
//! Pairs with liris's AGENT_RUNTIME_BIAS spec :82432 (Phase-6 audit invariants).

use alloc::string::String;
use alloc::vec::Vec;

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
    pub tier: crate::syscall::AccessTier,
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

/// Canonical max concurrent agents per spec (Step 118 benchmark target).
pub const AGENT_REGISTRY_MAX: usize = 10_000;

/// Heartbeat timeout — beyond this, supervisor marks `Failed`.
pub const HEARTBEAT_TIMEOUT_NS: u64 = 60 * 1_000_000_000; // 60 seconds

/// Gulp gate threshold per BEHCS-2000-msg canon.
pub const GULP_THRESHOLD: u32 = 2000;

/// Agent registry — in-memory v0.1; Phase-6 wave swaps to lock-free concurrent map.
pub struct AgentRegistry {
    entries: Vec<AgentEntry>,
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
        }
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

    /// Spawn primitive — v0.1 stub.
    /// Real impl mints BEHCS-1024 PID, appends to registry, emits AGENT_SPAWNED envelope.
    pub fn spawn(
        &mut self,
        _role: AgentRole,
        _vantage: VantageId,
        _tier: crate::syscall::AccessTier,
    ) -> Result<String, AgentErr> {
        if self.count() >= AGENT_REGISTRY_MAX {
            return Err(AgentErr::RegistryFull);
        }
        Err(AgentErr::Unimplemented)
    }

    /// Retire an agent by PID.
    pub fn retire(&mut self, _agent_pid: &str) -> Result<(), AgentErr> {
        Err(AgentErr::Unimplemented)
    }

    /// Heartbeat update for an agent.
    pub fn heartbeat(&mut self, _agent_pid: &str, _now_ns: u64) -> Result<(), AgentErr> {
        Err(AgentErr::Unimplemented)
    }
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
/// `crate::syscall::sys_time`).
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
    fn spawn_stub_returns_unimplemented() {
        let mut r = AgentRegistry::new();
        let res = r.spawn(
            AgentRole::Hermes,
            VantageId::Acer,
            crate::syscall::AccessTier::Public,
        );
        assert_eq!(res, Err(AgentErr::Unimplemented));
    }
}
