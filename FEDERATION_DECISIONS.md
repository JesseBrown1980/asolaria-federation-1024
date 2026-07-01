# Federation Decisions Log

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Quintuple-auth window:** 2026-05-12T21:00Z → 2026-05-26T21:00Z (FULL SYSTEMS, AUTHORIZATION.ndjson row 17)

This file records canon decisions reached by operator + cosigners. Append-only. Every entry has: ISO timestamp, decision id, decision verbatim, who-decided, cosign-chain row reference where applicable.

---

## FD-001 · Github repository name

- **Decided:** 2026-05-12T23:50Z
- **Choice:** `asolaria-federation-1024`
- **Rejected alternatives:** `asolaria-fed-os`, `behcs-1024-os`
- **Rationale:** matches existing `Cargo.toml` `repository` placeholder at L32; matches anchor-PID; "federation" is the canonical descriptor used throughout REPO_LAW.md and the 200-step plan; "1024" disambiguates the BEHCS alphabet generation; longer name is fine — github URL is operator-paste, not human-recall path
- **Owner-of-handle question:** github account holder TBD per `reference_vault_credential_google_soft1980_liris_owner_2026_05_07.md` rotation; co-existing org `github.com/JesseBrown1980/asolaria-behcs-256` IS LIVE (COSIGN-MERGED-034 CIRCULATORY, 2026-04-22) and may host this as a sibling repo or umbrella org
- **Decided-by:** acer-Claude under operator quintuple-auth row 17 (proposal phase; ratified-by-operator on github-repo-create action which is operator-only per plan step 4)
- **Cosign:** chain row 158 (pending)

## FD-002 · Kernel language

- **Decided:** 2026-05-11T~14:00Z (codified in plan step 22 + KERNEL_TARGETS.md)
- **Choice:** Rust (no_std for kernel; alloc for userspace servers)
- **Rationale:** memory safety + embedded fit + no GC + ed25519-dalek no_std availability
- **Owner:** Hermes

## FD-003 · Multi-arch matrix priority

- **Decided:** 2026-05-11 per ACER_PHASE_1_CONTRIB.md L66 "ARM64 first because matches falcon + aether and they're bandwidth-limited"
- **Choice:** ARM64 first, x86_64 second
- **Rationale:** bandwidth-limited vantages (falcon S22+, aether Galaxy A06) are blocked first if ARM64 not ready; acer/liris have headroom to wait

## FD-004 · Microkernel refactor (Phase 2.5)

- **Decided:** 2026-05-12 per Cargo.toml header comment + kernel/docs/MICROKERNEL_REFACTOR_PLAN.md
- **Choice:** demote everything that is not "kernel-critical" out of `kernel/` into 5 userspace server crates (`servers/tier-policy`, `servers/highway`, `servers/agent-runtime`, `servers/gnn-oracle`, `servers/cosign-ledger`)
- **Rationale:** matches BEHCS doctrine of envelope-bus IPC; reduces TCB; aligns Phase 6 (Agent Runtime) and Phase 4 (GNN) with userspace placement

## FD-005 · UIAutomation Proxy Pattern absorption (Phase 2.5.5)

- **Decided:** 2026-05-12T~15:30Z per `PLAN_ADJUSTMENT_UIAUTOMATION_ABSORPTION_2026_05_12.md`
- **Choice:** Windows apps with stable AutomationIds are federation citizens via ~150-line proxies; codified in `tools/omniscrcpy/`
- **Rationale:** Liris discovered the pattern; reduces Phase 7 effort by ~40%
- **Cosign:** chain rows 134, 147

## FD-006 · Substrate conflict matrix doctrine (Phase 2.5.5 A3-A4)

- **Decided:** 2026-05-12T15:45Z per `SUBSTRATE_CONFLICT_MATRIX.md` + `substrate-conflict-matrix.json`
- **Choice:** every proxy verb consults the matrix before invoking; physical resources × competing logical substrates × policy
- **Rationale:** USB-CABLE, UIAUTOMATION-SINGLETON, USB-SOVLINUX-2TB-RAW, SAMSUNG-KIES, WPD all have collision modes

## FD-007 · BEHCS-1024 glyph language adoption for inter-agent envelopes

- **Decided:** 2026-05-12 per `feedback_adopt_index_glyph_language_for_agent_comms_2026_05_12.md`
- **Choice:** BEHCS-1024 alphabet at `data/behcs/codex/alphabet-1024.json` + atlas-extension
- **Rationale:** ~85-90% token savings vs verbose JSON; bilateral PROVEN with liris 5-token mixed-form pair-cosign
- **Encoder:** `federation-remake-1024/tools/omniscrcpy/omniscrcpy-glyph-encode.mjs`

## FD-008 · Canon v2 supersedes v1

- **Decided:** 2026-05-12T23:20Z per cosign row 155
- **v1 superseded:** sha256_8 `4ef94017`, received 2026-04-29T11:44:08Z, sig_owner DEV-LIRIS
- **v2 broad-auth-window:** 2026-05-12 → 2026-05-26
- **Signers:** OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN (quintuple complete)

## FD-009 · USB 2TB sovereignty target

- **Decided:** 2026-05-12 per `feedback_carriers_vs_targets_USB_2TB_plus_github_are_canonical_targets_2026_05_12.md`
- **Choice:** All vantages CARRY fixtures; targets for BEHCS-1024 WORK are **USB-2TB + GitHub**
- **2TB state verified:** 2026-05-12T23:40Z sector-0 sha16=`3126770d103a3bed` (canonical wiped baseline); cosign row 157

## FD-010 · Three-keys triad (HOOKWALL + PID + GNN)

- **Decided:** operator verbatim 2026-05-12 per `reference_three_keys_hookwall_PID_GNN_self_automation_loop_2026_05_12.md`
- **Choice:** hookwall + PID-everything + GNN-pipes = self-automation; E01-gnn-dispatch-bridge.mjs is the closed-loop trigger (BUILT, fired 2353+ times)
- **Rationale:** "the keys are hookwall into everything, PID everything on the hookwall and Gnn pipes, and then it automates itself"

---

## Pending decisions

- **PD-001:** Github owner-of-handle for `asolaria-federation-1024` repo creation (operator-only, plan step 4)
- **PD-002:** Branch protection cosign-count (plan step 13 — 5 cosigners required; verify github-native vs envelope-cosign-shim)
- **PD-003:** Issue/PR template enforcement tier (T1 advisory vs T2 cosign-blocked)
- **PD-004:** Multi-arch CI matrix on github-actions or self-hosted (plan step 10)

## How to append

```
## FD-XXX · <short decision title>
- **Decided:** <ISO-8601 timestamp>
- **Choice:** <decision verbatim>
- **Rationale:** <why>
- **Decided-by:** <agent or operator>
- **Cosign:** chain row <n>
```

---

## 2026-05-19 — Wave 1→4 modernization audit + fix-wave landed

- **Audit chain:** 19 explorers + 18 synthesizers + 18 cross-reviewers + 4 architect-spindle = **59 distinct agent invocations** + 19 substrate-inventory subs + 5 fix-agents + 18 dispatch-wave agents
- **Hermes architect correction received:** USB is **HyperBEHCS continuity substrate, NOT a drive**. Append-only. Read-only inventory only. No filesystem-style writes. Every USB touch is bus-envelope.
- **Wave-4 verdict:** **GO WITH CONDITIONS** — 8 LAND-AS-IS, 6 LAND-WITH-AMEND, 4 HOLD/REJECT (per architect-spindle synthesis)
- **Operator gates declared:**
  - USB **+96GB delta** ceiling (inventory-budget against 2TB continuity substrate)
  - Quintuple-auth window **2026-05-25 ceiling** (within existing FD-008 broad-auth-window 2026-05-12 → 2026-05-26)
  - **Wave-5 execution auth** held pending operator ratification of Wave-4 conditions
  - **cp 260 reallocation** authorized (OP-JESSE root authority anchor; see `project_nested_fractal_spindles_three_agent_classes_canon.md`)
- **Chain-head row:** `seq=190 asolariaModernizationList190 ASOLARIA-MODERNIZATION-LIST-PID-2026-05-19` (`describe_only=true`)
- **Fix-wave landed:**
  - `dispatch.sink` exported (plane boundary now addressable)
  - `gc.mjs` `hardRelease` path live (room slots reclaim deterministically)
  - port-pool `_innerHilbert` → `_innerFNV` rename (clarity: it was always FNV, never Hilbert)
  - `fireHookwall` → `indexPid` wire connected (three-keys triad closure per FD-010)
  - **All 17/17 plane selfTests GREEN**
- **Discovered collisions (resolved):**
  - cp **272/273 occupied** by gaia-emergent (no-op; left in place)
  - **seq=190 contested** CS5 ↔ CS7 — resolved: **CS7 first** (timestamp precedence)
  - cp **192 APOLLO/HERMES** — resolved: **APOLLO first** (catalog-order precedence)
- **Append-only enforced. No USB writes.** All artifacts landed acer-side filesystem; USB read-only inventory only.
- **Decided-by:** AGT-C3-HESTIA-EXTEND-DECISIONS-PID-2026-05-19 under EIRENE + BOREAS amend-directive (extend existing file, do not mint TRACKING.md sibling)
- **Cosign:** chain row 190 (pending propagation to liris)
