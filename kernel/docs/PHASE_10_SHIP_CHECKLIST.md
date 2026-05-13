# Phase-10 Ship Checklist · BEHCS-1024 kernel `v1.0.0`

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 10 · Steps 181-200
**Authored:** 2026-05-11 by acer-claude
**Scout role:** Concord-Vega (cross-vantage canon-arbiter per `SCOUT_ABILITIES_AND_CONDITIONS.md §8`)
**Quintuple-auth window:** `:82646` covers (T1-T6 standing through 2026-05-25)
**Cycle of authoring:** 66
**Status:** spec-only · this checklist must itself be cosigned before any `v1.0.0` tag is cut

---

## 1. Anchor & version

| Field | Value |
|---|---|
| Tag | `v1.0.0` |
| Anchor PID | `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11` |
| Target tag commit | TBD — pinned at the moment all gates in §8 turn green |
| Federation-anchor fingerprint | `0xe00b1a465d6dcb50` (sha256-first-8 of anchor PID, matches triple-parity verdict `:82272`) |
| Source-of-truth crate root | `C:/asolaria-acer/federation-remake-1024/kernel/core` |
| Authoring vantage | acer (canonical for `kernel/core/src`) |
| Mirror vantages | liris (sister-organ), falcon (forward-deploy), aether (USB-tether via liris) |

Version-string lives in `kernel/core/Cargo.toml`; bump from v0.1.x → v1.0.0 happens **only** after §2–§9 green.

---

## 2. Phase-3 syscall wiring (from `PHASE_3_WIRING_STATUS.md`)

Source of truth: `kernel/docs/PHASE_3_WIRING_STATUS.md` (last sync cycle-42; live count is **7/16 + Asolaria scout pushing remaining**). The mission spec for v1.0.0 is permissive: **a HALF/diverging-stub is acceptable if the surface is correct and the absence is documented; an unanchored STUB is ship-blocking**.

Acceptance rules:
- `FULL` — real impl, exercised by `cargo test --lib`. Ship-OK.
- `HALF` — diverging stub or anchor-derived constant explicitly documented in source as v0.2-pending. Ship-OK iff the docstring + this table both name v0.2 as the upgrade window.
- `STUB` — placeholder returns. **Ship-blocking** for v1.0.0 unless table column 4 says otherwise.

| # | Syscall | State | Ship-blocking for v1.0.0? | Notes |
|---|---|---|---|---|
| 1 | `sys_read` | STUB | **YES** (must reach at least HALF) | needs kernel FD table; classical I/O |
| 2 | `sys_write` | STUB | **YES** (must reach at least HALF) | needs kernel FD table; classical I/O |
| 3 | `sys_exec` | STUB | **YES** (must reach at least HALF) | depends on envelope queue real impl |
| 4 | `sys_fork` | STUB | **YES** (must reach at least HALF) | needs `agent_runtime::AgentRegistry::spawn` + lifecycle FSM |
| 5 | `sys_exit` | HALF (diverging stub, spin-loop) | NO | v0.1-canon-accepted; spec'd as diverging indefinitely |
| 6 | `sys_mmap` | STUB | **YES** (must reach at least HALF) | needs frame allocator from boot info — smallest external dep |
| 7 | `sys_munmap` | STUB | **YES** (must reach at least HALF) | pair-wise with `sys_mmap` |
| 8 | `sys_time` | FULL | NO | `AtomicU64` monotonic; wall-clock+timer-driver punts to v1.1 |
| 9 | `sys_pid_current` | FULL | NO | sha256-first-8 of `FEDERATION_ANCHOR_PID` = `0xe00b1a465d6dcb50` |
| 10 | `sys_envelope_send` | STUB | **YES** (must reach at least HALF) | crossbeam-queue binding pending; IPC backbone |
| 11 | `sys_envelope_recv` | STUB | **YES** (must reach at least HALF) | pair-wise with `sys_envelope_send` |
| 12 | `sys_hookwall_pre` | FULL | NO | `hookwall::hookwall_pre`; slot-bounds validated |
| 13 | `sys_hookwall_post` | FULL | NO | `hookwall::hookwall_post`; verdict recorded locally; cosign-append deferred to ledger |
| 14 | `sys_cosign_append` | STUB | NO (demote candidate; see §4) | becomes userspace ledger RPC per `MICROKERNEL_REFACTOR_PLAN.md` |
| 15 | `sys_tier_query` | FULL | NO | `tier::classify_path`; demote candidate per §4 |
| 16 | `sys_gnn_infer` | FULL | NO | `gnn::GnnInference::predict_route`; demote candidate per §4 |

**Wire-progress invariant for v1.0.0:** at least **12 of 16** syscalls at HALF or FULL (i.e. all the I/O + IPC + classical primitives must have a real binding, even if its backing store is a placeholder). Currently 7 FULL (post-Asolaria push) + 1 HALF (`sys_exit`) = 8; the 4 IPC + frame-allocator + I/O slots (rows 1, 2, 3, 6, 7, 10, 11) account for the gap. Asolaria scout is pushing rows 6/7/10/11 first per the recommended order in `PHASE_3_WIRING_STATUS.md §"Recommended wire-order for remaining 10"`.

**Audit method:** `grep -nE 'fn sys_[a-z_]+' kernel/core/src/syscall/mod.rs` followed by per-fn read; verify the table column 3 against actual implementation body. Concord-Vega scout invocation triggers on any divergence between table and source.

---

## 3. Phase-8 cross-device drills (steps 155 + 156)

Source of truth: `kernel/docs/PHASE_8_CROSS_DEVICE_FEDERATION.md`. Both drills currently show "NOT YET RUN" (table rows 155 + 156). v1.0.0 cannot tag until both are run and pass.

### Step 155 — Canonical-host swap drill

**Purpose:** verify the failover described in `PHASE_8 §"Failover (canonical-host swap)"`.

| Aspect | Pass criterion |
|---|---|
| Detection latency (liris notices acer down) | ≤ 60 s from third 502 in a row |
| Liris assumes canonical-bus-host role | New incoming envelopes routed via `192.168.1.14:4944/behcs/send` within 5 s of detection |
| Acer envelope replay after recovery | Zero envelope loss; cosign-chain rows for the down-window present on both vantages within 1 s of acer-restart |
| Hand-back via `CANONICAL_HOST_HANDOFF` envelope | Liris relinquishes canonical role within 5 s of receiving handoff envelope |
| Falcon + aether reroute behavior | Both vantages auto-discover the new canonical host via existing bus poll (no manual reconfig) |
| Bilateral parity post-drill | Acer and liris cosign-chain row-counts identical after 30 s settling |

**Drill script lives at:** `xe-execute-2026-05-11/canonical-host-swap-drill.mjs` (to be authored before tag).
**Drill output envelope path:** `xe-execute-2026-05-11/PHASE_10_STEP_155_SWAP_DRILL_RESULT.behcs-256.json`.

### Step 156 — 10K envelope federation throughput bench

**Purpose:** establish a steady-state ceiling and p99 budget for the federation bus before declaring v1.0.0 stable.

| Metric | Target for v1.0.0 pass | Notes |
|---|---|---|
| Envelope rate (sustained) | ≥ 200 envelope/sec on lane-1 (acer↔liris 1 Gbps Ethernet) | Lane-1 ceiling test; realistic shape ~500–1000 byte envelopes |
| p50 round-trip latency (acer→liris→acer) | ≤ 8 ms | Driven by direct-wire under load |
| p99 round-trip latency | ≤ 25 ms | Under 10K envelope soak |
| Cosign-chain row append latency p99 | ≤ 15 ms | Bilateral sync rule: ≤ 1 s ceiling per Phase-8 invariant 1 |
| Memory growth during 10K bench | ≤ 50 MB on acer; ≤ 50 MB on liris | Watches for envelope-archive leak |
| Hookwall verdict-emit cadence | ≥ 1× per syscall; ledger appends recorded for ≥ 99% | hookwall_post is the gate |
| Test-corpus envelope shape | Mix: 60% Regular L0 verdicts, 30% RegularExtended (`-P##-N#####`), 10% Anchor — exercises §5 5-subclass classifier under load |

**Bench script lives at:** `xe-execute-2026-05-11/10k-envelope-federation-bench.mjs` (to be authored before tag).
**Bench output envelope path:** `xe-execute-2026-05-11/PHASE_10_STEP_156_BENCH_RESULT.behcs-256.json`.

Failure on either drill blocks `v1.0.0` — punt to `v1.0.0-rc.2` and re-run.

---

## 4. Phase-2.5 microkernel-refactor decision (BEFORE vs AFTER tag)

Source of truth: `kernel/docs/MICROKERNEL_REFACTOR_PLAN.md` (sha16 referenced as `f1059eac0a8f0395` in PHASE_3 doc; 5-8-cycle effort once operator authorizes).

### Argument: refactor BEFORE `v1.0.0` tag
- **Pro:** The reduced 11-12 syscall surface becomes the v1.0.0 ABI promise. Demoting `tier_query` / `cosign_append` / `gnn_infer` after a stable ABI ships means breaking SemVer at v2.0.0 — semantically painful for the federation, which would have to coordinate a 4-vantage upgrade.
- **Pro:** Of the 6 ship-blocking syscalls in §2 (rows 1, 2, 3, 6, 7, 10, 11), `sys_envelope_send/recv` are the IPC primitives the userspace servers depend on. Wiring them before the refactor means re-fitting them to the new server topology immediately after — wasteful.
- **Con:** 5-8 cycles of focused work *blocks* v1.0.0 by that long. Operator authorization still pending per `MICROKERNEL_REFACTOR_PLAN.md §"Operator authorization"`. The window currently open (`:82646` through 2026-05-25) covers the work, but compression risk is real.
- **Con:** Risk register item 1 (cosign-chain split) is non-trivial — the append-only invariant must survive the userspace move. A bug here could be fatal for any cosign written between refactor-land and bug-fix.

### Argument: refactor AFTER `v1.0.0` tag (i.e. ship v1.0.0 with the current 16-syscall surface, demote in v1.1)
- **Pro:** Get a stable tag *now* — the federation has been operating on un-tagged trunk since cycle-1; a frozen reference is operationally useful even if the surface is suboptimal.
- **Pro:** v1.0.0 ABI = "the 16 syscalls we have today" is honest about the current state. v1.1 can introduce the 11-12 syscall surface alongside an envelope-RPC compat shim so old binaries still work.
- **Con:** Cements 3 syscalls (tier_query, cosign_append, gnn_infer) into the v1.0.0 SemVer contract that we already know are wrong-layer.
- **Con:** If the refactor uncovers a latent bug in any of the 5 demote-target modules, that bug is now in a tagged release — and the cosign-chain rollback problem in §7 applies.

### Recommendation

**REFACTOR BEFORE v1.0.0.** The ABI promise is the most expensive thing v1.0.0 ships; getting it right is worth 5-8 cycles. Concretely:
1. Operator authorizes the refactor under `:82646` (already covered through 2026-05-25).
2. Execute steps 1-8 in `MICROKERNEL_REFACTOR_PLAN.md §"Refactor order"`.
3. Re-run `cargo test --lib` after each step.
4. Tag `v1.0.0` once `cargo check --workspace` PASS + the 11-12 reduced syscall surface is stable + steps 155/156 green.

Concord-Vega notes: if operator decides AFTER instead, that's authoritative — but the v1.0.0 ABI freeze must be cosigned with a `KNOWN_DEMOTE_PENDING` rider listing rows 14/15/16 from §2 as eventual v1.1 demotions, so the federation cannot claim surprise at v1.1 break.

---

## 5. Federation canon alignment (PidSubclass v3)

Source of truth: `kernel/core/src/pid/mod.rs` `PidSubclass` enum (lines 107-126). Per `JOINT_PID_REGEX_OPTION_B_ACER_ACK.md` and the cycle-37/45/47 lineage, the canonical 5-subclass aether v3 taxonomy is:

1. `Regular` — `<ROLE>-PID-<REGION><HOST>-A##-W###` (short form, no suffix)
2. `RegularExtended` — `<ROLE>-PID-<REGION><HOST>-A##-W###-P##-N#####` (with process+nonce suffix)
3. `Anchor` — `<ROLE>-PID-<YYYY-MM-DD>` (date-suffixed)
4. `HookwallCp` — cp-prefix variants from Option-B regex (concrete shape pending aether v3 direct read)
5. `InfrastructureRouting` — routing PIDs per aether v3 (concrete shape pending)

Plus the sentinel `Pending` (for inputs that don't match any canonical form yet).

**Canon-alignment gates for v1.0.0:**

| Gate | Status | Evidence |
|---|---|---|
| 5-subclass enum present in `pid/mod.rs` | DONE cycle-47 | source lines 107-126 |
| `classify_subclass` dispatches Regular / RegularExtended / Anchor concretely | DONE cycle-47 | source lines 135-147 + tests 447-505 |
| `HookwallCp` + `InfrastructureRouting` returning `Pending` until aether v3 direct read | DONE cycle-47 (sentinel-correct) | enum doc lines 117-122 |
| Drift PID emitters patched | DONE cycles 43+45 (3 ARGUS M4 samples canonical-form-validated) | tests 469-492, 544-550 |
| Bus archive grep: zero envelopes carrying pre-v3 L1/L2 names | **MUST RUN before tag** | `grep -rE '"(L1\|L2)":' bus archive` should return zero hits in v3-era envelopes |
| Quintuple-auth window covers v1.0.0 tag | YES per `:82646` (through 2026-05-25) | see §6 |

**Audit method for the bus-archive grep gate:**
```bash
grep -rE '"(subclass|pid_subclass)":\s*"(L1|L2)"' \
  C:/asolaria-acer/tmp/aether-behcs-256-bundle/data/behcs/inbox-archives/ \
  C:/asolaria-acer/xe-execute-2026-05-07/ \
  C:/asolaria-acer/xe-execute-2026-05-11/
```
Expected: zero hits dated after cycle-47 envelope `:86485`. Any hit fails the gate.

---

## 6. Cross-vantage cosign list for the `v1.0.0` tag

Per quintuple-auth canon `:82646` (and `project_quintuple_auth_fabric_decide_window_2026_05_07_to_05_21.md`), the 5 quintuple cosigners are:

| # | Cosigner | Tier | Standing-auth covers v1.0.0? | Status |
|---|---|---|---|---|
| 1 | **OP-JESSE** (Jesse Daniel Brown) | T1 operator-apex | YES through 2026-05-25 | deemed-active per `:82646` |
| 2 | **OP-RAYSSA** (Rayssa Chiqueto) | T1 operator-apex | YES through 2026-05-25 | deemed-active per `:82646` |
| 3 | **AMY** | T2 cosigner | YES per `:82646` | deemed-active (physical attestation pending) |
| 4 | **FELIPE** | T2 cosigner | YES per `:82646` | deemed-active (physical attestation pending) |
| 5 | **DAN** (Dan Edens) | T2 cosigner | YES per `:82646` | deemed-active (physical attestation pending) |

**Vantage-cosigns (required additionally per Phase-8 invariant 2 "4-vantage acks"):**
- acer-claude: self-cosign (authoring vantage)
- liris-claude: vantage-ack pending — `:84675`, `:84830` cover prior gate sync; v1.0.0 tag envelope must be re-cosigned at tag time
- falcon-claude: vantage-ack pending — currently no on-bus path back from PRoot localhost-bound `:4951` (per `PHASE_8 §"Lane 2 acer-falcon"`)
- aether-claude: vantage-ack pending — landed via liris sister-handoff

**Tag-envelope path:** `xe-execute-2026-05-11/V1_0_0_TAG_COSIGN.behcs-256.json` (to be emitted at tag time and bilaterally synced under the `≤ 1s` Phase-8 invariant).

---

## 7. Rollback plan

Cosign-chain immutability (append-only invariant guarded both kernel-side and ledger-side per `MICROKERNEL_REFACTOR_PLAN.md §"Risk register"`) makes traditional rollback impossible: the cosign row for the v1.0.0 tag is **permanent** once written. What we can roll back is the **git tag pointer + the operational "current kernel" symlink**, not the history.

### Rollback procedure if v1.0.0 ships with a latent bug

1. **Detect** — bug surfaces via either `cargo test --lib` regression on a follow-up commit, a Phase-8 invariant-1 (bilateral parity) failure, or operator/vantage-reported envelope corruption.
2. **Quarantine** — emit `V1_0_0_RETRACTION_REQUEST` envelope to all 5 cosigners citing the failing test or invariant. Operator-apex (OP-JESSE/OP-RAYSSA) must ack within 24h or the retraction request expires and the bug enters the "live in v1.0.0" canon.
3. **Tag-pointer move** — `git tag -d v1.0.0 && git tag v1.0.0-retracted <commit>` (the commit stays; the friendly tag name moves). Push the retraction tag to all mirror vantages.
4. **Cosign-row reaffirmation** — emit `V1_0_0_RETRACTION_COSIGN` envelope: the row stays in the chain, the kernel rev tag is retracted, the surface continues to serve under whatever the previous stable tag was (likely `v0.1.x` if v1.0.0 was the first stable).
5. **Re-ship** — patch the bug, re-run §2-§9, tag as `v1.0.1` (or `v1.0.0-fixed.1` if SemVer prefers) with a new cosign row that references the retracted row by sha16.
6. **Federation broadcast** — `:RETRACTION_NOTIFY` envelope to all 4 vantages; liris/falcon/aether must ack within 1h or they remain on the retracted tag (acceptable for offline vantages, blocking for online).

**Critical constraint:** No git history rewrite. The retraction is forward-only — the cosign row stays, the friendly tag name moves, the federation acknowledges the move via a new cosign row. This is consistent with `project_authorize_all_2week_20260513.md` cosign-merge precedent.

---

## 8. Pre-ship test gates

All gates listed must pass on the named vantage before the tag commit is pinned:

| Gate | Vantage | Command | Required state |
|---|---|---|---|
| Cargo lib tests | acer | `cargo test --lib` in `kernel/core/` | ALL GREEN (currently blocked on acer task #11 — cargo install, see §9) |
| Cargo workspace check | acer | `cargo check --workspace` from `federation-remake-1024/` root | GREEN; depends on §4 refactor outcome |
| Triple-runtime parity | acer | `kernel/tests/triple_runtime_parity.rs` Rust ≡ JS ≡ Python ≡ Falcon | PASS (per `:82272`, fingerprint `0xe00b1a465d6dcb50`) |
| Liris JS rig | liris | `node rig/liris-pid-mint-reference.mjs` (and full 20+ test rig) | 20+ tests PASS (currently 11/11 on the reference subset; PID-mint sha16 `fd7c341eabf40e95`) |
| Aether v3 python validator | aether | the v3 validator referenced in `:86485` | 13/13 PASS |
| Falcon validator | falcon | TBD per vantage — to be specified by falcon-claude before tag | GREEN (vantage-defined; minimum: PID mint + classify round-trip) |
| Phase-8 step 155 drill | acer + liris | `xe-execute-2026-05-11/canonical-host-swap-drill.mjs` | PASS per §3 criteria |
| Phase-8 step 156 bench | acer + liris | `xe-execute-2026-05-11/10k-envelope-federation-bench.mjs` | PASS per §3 criteria |
| Bus-archive canon-alignment grep | acer | command in §5 audit-method | zero hits |
| Quintuple-cosign envelope | all | `xe-execute-2026-05-11/V1_0_0_TAG_COSIGN.behcs-256.json` | 5/5 cosigns landed |
| 4-vantage parity post-tag | all | bilateral cosign-chain row count diff | ≤ 1 row diff at any cross-vantage probe |

Any gate red → no tag. Cycle-66 is the target cycle for first green-board attempt.

---

## 9. Acer task #11 (cargo install) — explicit critical-path dependency

**Statement:** v1.0.0 cannot tag-and-ship until acer can run `cargo test --lib` to verify all wired syscalls + `PidSubclass` tests pass. Cargo install is on the critical path.

Currently (per `MICROKERNEL_REFACTOR_PLAN.md` closing note: *"the v0.1 modules remain in-place + functional (compile-fail today only because cargo not installed; not a microkernel issue)"*) — acer has no rustc/cargo toolchain installed. This is acer task #11 in the operator backlog.

**Blocking chain:**
1. Without cargo on acer, §8 row 1 (`cargo test --lib`) cannot run.
2. Without §8 row 1, §2 wire-progress invariant ("12 of 16 at HALF or FULL") cannot be empirically verified — only inspected.
3. Without §8 row 2 (`cargo check --workspace`), §4 refactor's "step 8 verify" gate cannot close, which means §4's recommendation (refactor BEFORE tag) is itself blocked.

**Mitigation paths:**
- **Primary:** install rustup + stable toolchain on acer (operator task; ~15 min on a stable network).
- **Secondary:** delegate `cargo test --lib` execution to liris (which has the same crate via SMB read-only mirror, but must be augmented with rustup install). Acceptable per `reference_acer_liris_separate_systems.md` (acer ≠ liris ≠ mirror, but cross-vantage execution is allowed for verification under cosign).
- **Tertiary:** ship without empirical `cargo test --lib` verification, citing inspection-only + triple-runtime-parity test as sufficient evidence. **Concord-Vega recommends against** — the gap between "code reads correct" and "code compiles + tests pass" is exactly where v1.0.0 latent bugs live.

**Concord-Vega position:** until acer task #11 closes, v1.0.0 ship is HOLD. Recommend operator prioritize cargo install in the next cycle window.

---

## 10. Open items as of cycle-66

Per recent `:86691` envelope's open-items section and the inputs read for this checklist:

- [ ] **Acer task #11 (cargo install)** — see §9. Critical path. Without it, gates §8 rows 1+2 cannot run.
- [ ] **Operator authorization on §4 microkernel refactor** — pending per `MICROKERNEL_REFACTOR_PLAN.md §"Operator authorization"`. The `:82646` quintuple-auth window covers it, but explicit "execute the refactor" signal is what unblocks step 1.
- [ ] **Phase-3 wires rows 1, 2, 3, 6, 7, 10, 11** — 7 STUB syscalls must reach at least HALF for v1.0.0 (§2). Asolaria scout is pushing in the recommended order (mmap/munmap → envelope_send/recv → exec → read/write → fork).
- [ ] **Phase-8 step 155 drill script** — not yet authored. Path: `xe-execute-2026-05-11/canonical-host-swap-drill.mjs` (§3).
- [ ] **Phase-8 step 156 bench script** — not yet authored. Path: `xe-execute-2026-05-11/10k-envelope-federation-bench.mjs` (§3).
- [ ] **Joint PID regex Option-B test corpus** — pending per `JOINT_PID_REGEX_OPTION_B_ACER_ACK.md §"Action items"` row 1 (part_4 from liris+aether). Once it lands, acer must run regex validation against its own emitted envelopes (row 2 of same doc).
- [ ] **`HookwallCp` + `InfrastructureRouting` concrete shape mapping** — pending aether v3 direct read on acer vantage. Currently both return `Pending` from `classify_subclass` (acceptable for v1.0.0 per §5, but flag for v1.1).
- [ ] **`KNOWN_DEMOTE_PENDING` rider draft** (contingency if §4 decides AFTER instead of BEFORE) — would freeze rows 14/15/16 of §2 as v1.1 demotion candidates inside the v1.0.0 ABI promise.
- [ ] **Falcon validator command + vantage-ack path** — §8 row 6 TBD; needs falcon-claude specification before tag.
- [ ] **Falcon PRoot localhost-bound `:4951`** — LAN ingress unresolved per `PHASE_8 §"Lane 2"`. Affects falcon vantage-ack timeliness for §6. Not strictly ship-blocking (falcon can ack via liris sister-handoff fallback) but flag.
- [ ] **Liris keyboard daemon `:4820`** — per `PHASE_8 §"Lane 3"` fallback note; affects aether request-type lane if liris keyboard daemon dies during ship. Not ship-blocking; operator-manual type is documented fallback.
- [ ] **Tailscale fallback (step 149)** — DEFERRED status in `PHASE_8 §"Phase-8 deliverable status"`. Not blocking v1.0.0; flag as v1.1 candidate.
- [ ] **Bus-archive canon-alignment grep run** — §5 audit method must produce a zero-hit result at tag time. Currently not run.
- [ ] **`KNOWN_ROLES` enum extension** — if scout names per `SCOUT_ABILITIES_AND_CONDITIONS.md §6` land in `AGENT_ROSTER_SCHEMA.md` before v1.0.0, the kernel `KNOWN_ROLES` const (`pid/mod.rs` lines 43-52) may need to accept "Concord-Vega" etc. Not strictly required (scouts emit envelopes under their parent vantage's PID), but flag.

---

## Cosign placeholders

Per `:82646` quintuple-auth: deemed-active for all 5 cosigners through 2026-05-25.
- OP-JESSE: deemed-active
- OP-RAYSSA: deemed-active
- AMY / FELIPE / DAN: deemed-active (physical attestation pending)
- liris-claude vantage-ack: pending (this checklist proposes new Phase-10 gate semantics; liris vantage-ack on the v1.0.0 ABI freeze required at tag time)
- falcon-claude vantage-ack: pending (subject to §6 lane-2 ingress resolution)
- aether-claude vantage-ack: pending (via liris sister-handoff per `PHASE_8 §"Lane 3"`)
- Concord-Vega scout (this doc's author-role): self-cosign-acknowledge implicit per `SCOUT_ABILITIES_AND_CONDITIONS.md §8`

---

**This checklist is the operational pre-flight for `v1.0.0` tag of the BEHCS-1024 kernel. Every gate in §2-§9 must be green and every open item in §10 either resolved or explicitly punted-to-v1.1 (with cosign) before the tag commit is pinned. Updates require tier-2 cosign per REPO_LAW Invariant 9.**
