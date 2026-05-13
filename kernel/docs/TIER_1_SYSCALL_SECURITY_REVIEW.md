# Tier-1 Syscall Surface Security Review

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 2 · Step 36
**Status:** v0.1 review · pre-implementation freeze gate before Phase-3 (steps 41-60) full-wire
**Authored:** 2026-05-12 by acer-Claude under FULL SYSTEMS quintuple-auth (AUTHORIZATION.ndjson row 17)
**Cross-reference:** `kernel/docs/USERSPACE_ABI.md`, `kernel/docs/PHASE_3_WIRING_STATUS.md`, `kernel/core/src/syscall/mod.rs`, `REPO_LAW.md` Invariants 5-10

---

## 1. Scope

Reviews the 16 canonical Phase-2 syscalls (`sys_*`) at their current wire-state (cycle-66: 6 FULL + 8 HALF + 1 doc-only + 1 diverging stub). For each: threat-model bullet, current mitigation, residual risk, Phase-3 hardening action.

This is a **pre-implementation freeze** review — the purpose is to attach explicit doctrine to each syscall BEFORE userspace consumers depend on observable behaviors that may turn out to be unsafe.

## 2. Threat model assumptions

- **Untrusted caller:** every syscall invocation is treated as potentially adversarial. Even tier-1 callers can be compromised.
- **Trusted hookwall:** `sys_hookwall_pre`/`sys_hookwall_post` are themselves the trust boundary; if they're bypassable, the model collapses (see §13 Hookwall Self-Protection).
- **Trusted cosign-chain:** chain is append-only, sha-linked, ed25519-signed. Adversary cannot forge or reorder rows without operator key compromise.
- **Adversary cannot run kernel-mode code** — that would invalidate everything; deferred to Phase-2 step 23 (UEFI minimal stub hardening).

## 3. Syscall-by-syscall review

### sys_read (1) · HALF-WIRED
- **Threat:** path-traversal via crafted FD; length-overflow read past buffer.
- **Mitigation now:** rejects `len > buf.len()` → `EINVAL`. FD-to-path resolution not yet wired (Unimplemented at full-wire).
- **Residual risk:** when VFS lands (v0.2), FD table must be per-process + tier-tagged. Cross-tier FD inheritance is BAD.
- **Phase-3 hardening:** FD table entries carry tier-tag; `sys_read` must verify caller-tier ≥ FD-tier else `EPERM`. Add cosign-chain row on every cross-tier read attempt.

### sys_write (2) · HALF-WIRED
- **Threat:** TOCTOU between FD-permission check and durable write; arbitrary write-past-EOF; covert channel via partial-write timing.
- **Mitigation now:** rejects empty buffer; valid → Unimplemented.
- **Residual risk:** without atomic write-or-rollback, partial writes are observable.
- **Phase-3 hardening:** writes commit via CAS storage driver only; partial-write is impossible (write-or-fail). Append cosign-chain row on every T2+ write.

### sys_exec (3) · HALF-WIRED
- **Threat:** envelope-bomb DoS; malformed envelope panics dispatcher; cross-tier privilege escalation via crafted envelope.type.
- **Mitigation now:** rejects `<32 byte` envelope. Valid → Unimplemented (queue v0.2).
- **Residual risk:** 32-byte minimum is too low — empty payload envelopes can still flood the queue.
- **Phase-3 hardening:** rate-limit via hookwall slot 0 (`sliding-1s, 100/sec` T1 budget); validate `envelope.type` against type registry before queueing; reject unknown types with `EINVAL`. Cosign-chain row on every `T2+` exec.

### sys_fork (4) · DOC-ONLY
- **Threat:** fork-bomb; PID exhaustion of BEHCS-1024 anchor space.
- **Mitigation now:** Unimplemented; no scheduler yet.
- **Residual risk:** when scheduler lands, BEHCS-1024 PID space is 2^60 — exhausting it via fork-bomb takes more time than universe age IF every fork mints a fresh PID. BUT if PID is recycled, ABA attacks become possible.
- **Phase-3 hardening:** fork mints fresh BEHCS-1024 PID via `mint_behcs1024_pid_60()`; no PID recycling. Per-process fork-rate hookwall slot. Cosign-chain row only on fork-rate exceeded (not every fork — too noisy).

### sys_exit (5) · DIVERGING STUB
- **Threat:** zombie-process accumulation; resource leak.
- **Mitigation now:** spin-loop (acceptable for v0.1 — single-task kernel).
- **Residual risk:** when scheduler lands, exit must reclaim PID + close FDs + signal supervisor (omnispindle).
- **Phase-3 hardening:** exit emits `EVT-PROCESS-EXITED` to bus; supervisor reaps; FD cleanup mandatory; cosign-chain row only on abnormal exit (signal-killed, panic).

### sys_mmap (6) · HALF-WIRED
- **Threat:** address-space exhaustion; W+X (write+execute) page abuse; mmap of driver MMIO bypassing driver-model envelopes.
- **Mitigation now:** rejects `len==0` + `prot==0`. Valid → Unimplemented.
- **Residual risk:** W^X enforcement not yet specified.
- **Phase-3 hardening:** W^X strict — `prot` cannot be `WRITE|EXEC` simultaneously, return `EINVAL`. MMIO mapping requires `DRIVER_REQUEST_*` envelope path, not `sys_mmap`. Per-process VA budget capped at 4 TiB (BEHCS-1024 native cap). Cosign-chain row on T2+ allocations.

### sys_munmap (7) · HALF-WIRED
- **Threat:** double-free, use-after-free (UAF) by remapping freed range.
- **Mitigation now:** rejects null pointer + zero length.
- **Residual risk:** UAF window between munmap and TLB shootdown.
- **Phase-3 hardening:** TLB shootdown synchronous on multi-core; punt to v0.2 with frame allocator. Cosign-chain row only on T3+ unmap.

### sys_time (8) · WIRED ✓
- **Threat:** timing side-channel for cosign-chain (timestamp prediction).
- **Mitigation now:** monotonic `AtomicU64` counter, NOT wall-clock — timestamp is per-boot relative.
- **Residual risk:** monotonic is fine for ordering, useless for cosign-chain wall-clock timestamps. v0.2 wiring to RDTSC/CNTVCT must NOT expose nanosecond precision to T1 callers (covert channel).
- **Phase-3 hardening:** T1 callers see microsecond precision max; T2+ see nanosecond. Cosign-chain timestamps come from operator-witness signed time, not kernel-local. **Status: acceptable for v0.1.**

### sys_pid_current (9) · WIRED ✓
- **Threat:** PID-spoofing via syscall return manipulation (mitigated by syscall ABI integrity).
- **Mitigation now:** returns sha256(`FEDERATION_ANCHOR_PID`)[:8] as u64 = `0xe00b1a465d6dcb50` — matches 4-runtime triple-parity verdict.
- **Residual risk:** when multi-process lands, each process needs distinct PID. Current impl returns federation-anchor only.
- **Phase-3 hardening:** v0.2 per-process PID from BEHCS-1024 minter; verify monotonicity within boot. **Status: acceptable for v0.1.**

### sys_envelope_send (10) · HALF-WIRED
- **Threat:** envelope queue flooding; T1 caller emitting T3 envelopes by lying about type.
- **Mitigation now:** rejects `<32 byte` envelope.
- **Residual risk:** envelope-type-vs-caller-tier mismatch unchecked.
- **Phase-3 hardening:** dispatcher validates `envelope.type` tier against caller tier; reject mismatch. Sliding-window rate limit per tier (T1 100/sec, T2 10/sec, T3 1/sec per `behcs-bus.js` canon). Cosign-chain row on every T2+ send.

### sys_envelope_recv (11) · HALF-WIRED
- **Threat:** info leak — T1 caller reading T2+ envelopes intended for higher tier.
- **Mitigation now:** rejects empty out_buf.
- **Residual risk:** receive must enforce per-tier filtering on `to` field.
- **Phase-3 hardening:** envelope `to` field must match caller PID exactly OR caller-tier ≥ destination-tier. Cosign-chain row on every cross-tier read.

### sys_hookwall_pre (12) · WIRED ✓
- **Threat:** hookwall self-bypass; pre-hook returning fake PROCEED via spoofed `HookContext`.
- **Mitigation now:** `HookContext` synthesized from envelope bytes prefix (byte[0]=slot, byte[1]=syscall_no); pure context-classification, zero state.
- **Residual risk:** if userspace can craft envelope-bytes that synthesize the wrong slot/syscall_no, classification is wrong.
- **Phase-3 hardening:** hookwall slot lookup is read-only from kernel-resident policy table; userspace cannot mutate. Add slot-range bounds check (slot < 16). Cosign-chain row on every BLOCK verdict (already in canon).

### sys_hookwall_post (13) · WIRED ✓
- **Threat:** verdict-replay attack — userspace replays old PROCEED to skip current hookwall decision.
- **Mitigation now:** verdict recorded locally; cosign-chain append deferred (v0.2).
- **Residual risk:** without cosign-chain append, replay is undetectable until v0.2.
- **Phase-3 hardening:** every post-hook verdict appends to cosign-chain with `prev_sha` link; replay produces broken chain. **Highest-priority Phase-3 gap.**

### sys_cosign_append (14) · HALF-WIRED
- **Threat:** chain-fork — caller appends row with wrong `prev_sha` creating divergent chain.
- **Mitigation now:** rejects `<16 byte` row.
- **Residual risk:** `prev_sha` link not yet validated; valid → Unimplemented.
- **Phase-3 hardening:** `cosign-ledger` server (Phase-2.5 demote) validates `prev_sha` matches chain-tip atomically; reject divergent appends with `EAGAIN`. Concurrent appends serialize via mutex on chain-tip pointer. Append rate-limited per tier (T2 100/min, T3 10/min).

### sys_tier_query (15) · WIRED ✓
- **Threat:** path-spoofing — caller queries arbitrary path to learn classification.
- **Mitigation now:** `tier::classify_path` pure heuristics on UTF-8 path; `UnclassifiedDefault → Public`.
- **Residual risk:** read-only info leak (caller learns path-tier) is acceptable — path-tier is not a secret.
- **Phase-3 hardening:** no change needed. **Status: acceptable for v0.1.**

### sys_gnn_infer (16) · WIRED ✓
- **Threat:** GNN model-load swap via untrusted blob; output manipulation by crafted input.
- **Mitigation now:** `GnnInference::predict_route` with deterministic fallback when model unloaded; encodes `RoutingDecision` as 1-byte enum (0..6).
- **Residual risk:** model-swap path (when implemented) must cosign-verify the model PID.
- **Phase-3 hardening:** model-load envelope `GNN_MODEL_LOAD` requires T3 cosign (5-signer); model PID is content-addressed. Per Phase 4 step 73 (model-swap protocol).

## 4. Cross-cutting findings

### F1 · Replay defense is the #1 Phase-3 gap
`sys_hookwall_post` records verdict locally but doesn't append to cosign-chain yet. Until that's wired, hookwall verdicts can be replayed. **Action:** Phase-3 step 45 (cosign-chain integration: every BLOCK verdict appended) must extend to ALL verdicts, not just BLOCK.

### F2 · Tier-tagging is the dominant invariant
6 of 16 syscalls have "T2+ → cosign-chain row" hardening actions. Without robust per-process tier-tagging at the kernel level (v0.2), all of these fail open. **Action:** Phase-3 step 46 (per-tier PID-gate enforcement) is on the critical path.

### F3 · Envelope.type is an authorization handle
`sys_envelope_send` / `sys_envelope_recv` use `envelope.type` as the primary routing key. The type registry MUST be kernel-resident; userspace cannot extend it without T3 cosign. **Action:** Phase-3 step 47-49 (Tier-1/T2/T3 hook surfaces) must lock the type registry.

### F4 · CAS-only storage closes 3 attack classes
Block-addressable storage would expose: TOCTOU, partial-write, covert-channel-via-block-pattern. CAS by content-hash makes all three impossible. **Action:** Phase-2 step 35 (BEHCS-256 CAS filesystem) is correctly prioritized; do not regress to block API.

### F5 · No syscall currently leaks operator keys
All keys live in `data/vault/owner/ed25519/*.private.b64`; kernel reads them at boot for `sign_gate`. No syscall returns key material to userspace. **Verified:** grep'd for key-fetch surface; none exists. **Status: GREEN.**

## 5. Pre-implementation freeze decisions

Locked decisions for Phase-3 wiring (no rollback without quintuple cosign):

| ID | Decision | Rationale |
|---|---|---|
| SR-001 | 16-syscall surface MAX | Anything beyond goes through envelope-bus, not new syscalls |
| SR-002 | W^X strict on `sys_mmap` | No write+executable pages, ever |
| SR-003 | BEHCS-1024 PID fresh-mint on fork | No PID recycling; ABA-defense |
| SR-004 | `sys_hookwall_post` cosign-chain append MANDATORY in v0.2 | Replay defense |
| SR-005 | T3 envelopes require quintuple cosign at issue | Already in REPO_LAW Invariant 9 |
| SR-006 | Type registry kernel-resident | Userspace cannot extend without T3 cosign |
| SR-007 | CAS-only storage (no block API) | Closes TOCTOU/partial-write/covert-channel |
| SR-008 | MMIO via DRIVER_REQUEST_* only (no mmap) | Closes driver-bypass attack surface |

## 6. Verify gate

- Path: `kernel/docs/TIER_1_SYSCALL_SECURITY_REVIEW.md`
- Phase: 2, step 36
- Status: pre-implementation freeze review COMPLETE for cycle-66 wire-state
- Cosign-chain reference: row 159 companion (canon-drift-fix + Phase-2 advance batch)
- Next review trigger: any change to the 16-syscall surface, or any new tier introduction

## 7. Open follow-ups

- Re-review after Phase-3 cycle-67+ (when 8 HALF wires become FULL)
- Re-review after Phase-2.5 microkernel refactor lands (`sys_cosign_append` + `sys_gnn_infer` demote to userspace via envelope IPC — review the demotion path doesn't open new attack surface)
- Phase-4 review: add GNN model-load attack vectors when step 73 lands
