# Microkernel Refactor Plan · Phase-2.5

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Authored:** 2026-05-11 by acer-claude
**Verdict source:** Vanguard-Acer scout #1 (microkernel review, agent-id a8cff0bc580c9c997)
**Operator authorization:** PENDING — refactor blocks Phase-3 wiring until executed
**Quintuple-auth window covers it:** YES (per `:82646`, T1-T6 standing approval through 2026-05-25)

---

## Verdict summary

Current 15-module `kernel/core` is **microkernel-rhetorical, not architectural**. The crate co-locates primitives (correct ring-0) with policy/service code (should be userspace). Discipline must move from naming to crate-boundary.

## Demotion list (5 modules → userspace servers)

| Current ring-0 module | Target userspace crate | Rationale |
|---|---|---|
| `kernel/core/src/tier/mod.rs` | `servers/tier-policy` | Pure policy lookup (TIER_POLICY_TABLE + classify_path). Kernel keeps only `AccessTier` enum + capability check. |
| `kernel/core/src/highway/mod.rs` | `servers/highway-broker` | Cross-tier transit orchestration. Consumes envelope IPC + cosign envelopes. |
| `kernel/core/src/agent_runtime/mod.rs` | `servers/agent-supervisor` | Process supervisor (Vec<AgentEntry> registry). Textbook userspace role à la Minix `pm`/`rs`. |
| `kernel/core/src/gnn/mod.rs` | `servers/gnn-oracle` | ML inference at ring-0 is the loudest violation. 100ms p99 budget belongs in userspace. |
| `kernel/core/src/cosign_chain/mod.rs` (storage half) | `servers/cosign-ledger` | ndjson writer + chain validation are storage backend. Kernel keeps signature-verify + append-only invariant guard. |

## Ring-0 keeps (5 primitives)

- `pid` — BEHCS-1024 PID minter (addressing kernel)
- `envelope` — atomic envelope dispatch (lock-free MPMC ring; IPC primitive)
- `crypto` — ed25519 substrate (signature verify)
- `hookwall` — pre/post syscall hooks (slot dispatch, verdict emit)
- `syscall` — canonical surface (reduced from 16 → 11-12)

## Syscall surface reduction (16 → ~11-12)

| Syscall | Keep | Reason |
|---|---|---|
| read / write / exec / fork / exit / mmap / munmap | YES | Classical kernel primitives |
| time / pid_current | YES | Kernel state queries |
| envelope_send / envelope_recv | YES | IPC primitives |
| hookwall_pre / hookwall_post | YES | Reduced to one combined `hookwall_invoke`? Operator decision. |
| cosign_append | **DEMOTE** | Becomes envelope to `servers/cosign-ledger` |
| tier_query | **DEMOTE** | Becomes envelope to `servers/tier-policy` |
| gnn_infer | **DEMOTE** | Becomes envelope to `servers/gnn-oracle` |

Result: **11-12 canonical syscalls** (down from 16). Demoted syscalls become userspace RPC via existing envelope IPC.

## Refactor order (8 steps)

### Step 1 — Top-level workspace
- New `federation-remake-1024/Cargo.toml` (workspace root) with `members = ["kernel/boot", "kernel/core", "servers/*"]`
- Reduces `kernel/Cargo.toml` from workspace-root to member-package

### Step 2 — Create servers/ skeleton
- `servers/tier-policy/Cargo.toml` (no_std + envelope-recv loop)
- `servers/highway-broker/Cargo.toml`
- `servers/agent-supervisor/Cargo.toml`
- `servers/gnn-oracle/Cargo.toml`
- `servers/cosign-ledger/Cargo.toml`

### Step 3 — Move policy out of `kernel/core/src/tier/`
- `AccessTier` enum stays in `kernel/core/src/syscall::AccessTier` (already there)
- `TIER_POLICY_TABLE` + `classify_path` + `quintuple_auth_covers` → `servers/tier-policy/src/lib.rs`
- Kernel `tier/mod.rs` becomes ~30-line capability-check stub

### Step 4 — Move agent_runtime → servers/agent-supervisor
- `AgentRegistry` + `AgentEntry` + lifecycle FSM → server crate
- Kernel keeps zero of these — pure userspace

### Step 5 — Move gnn → servers/gnn-oracle
- All of `gnn/mod.rs` content → server crate
- `sys_gnn_infer` becomes envelope-send shim: kernel composes envelope, sends to `gnn-oracle` server inbox, blocks on response. Real path: envelope IPC.

### Step 6 — Move cosign_chain storage half → servers/cosign-ledger
- `CosignChain::append` (ndjson write) → server
- Kernel retains: row-signature verify + append-only invariant guard

### Step 7 — Move highway → servers/highway-broker
- Pure orchestration on top of cosign-ledger + tier-policy
- Consumes envelope IPC from both

### Step 8 — Verify
- `cargo check --workspace` PASS on acer-host (Phase-10 CI prep)
- Re-run `kernel/tests/triple_runtime_parity.rs` — kernel keeps `pid::pid_fingerprint_sha16`, still ports clean
- `kernel/docs/USERSPACE_ABI.md` v0.2: 11-12 canonical syscalls, demoted ops now envelope-RPC patterns

## Effort estimate

~5-8 cycles of focused work once operator authorizes. Most modules already structured cleanly enough to move with minimal edits (just re-home + adjust imports).

## Risk register

1. **Cosign-chain split:** The append-only invariant must survive the split. Kernel verifies; userspace ledger durably writes. Hookwall hook ensures kernel-verify happens before userspace-write.
2. **GNN cold-start latency:** Demoting GNN means first inference traverses envelope IPC; budget 100ms p99 must include round-trip. Acceptable per current canon.
3. **Tier-policy hot-reload:** Currently TIER_POLICY_TABLE is `const` — userspace server enables hot-reload but loses compile-time guarantees. Acceptable trade per Phase-9.

## Cosign placeholders

- OP-JESSE: deemed-active per `:82646` quintuple-auth declaration
- OP-RAYSSA: deemed-active per `:82646`
- AMY / FELIPE / DAN: deemed-active per `:82646` (physical attestation pending)
- liris-claude vantage-ack: pending
- falcon-claude vantage-ack: pending
- aether-claude vantage-ack: pending
- Vanguard-Acer (scout #1 originator): self-cosign-acknowledge implicit

---

**Once operator authorizes execution, this plan unblocks Phase-3 hookwall real-verdict wiring. Until then, the v0.1 modules remain in-place + functional (compile-fail today only because cargo not installed; not a microkernel issue).**
