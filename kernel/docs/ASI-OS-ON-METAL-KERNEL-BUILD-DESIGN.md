# Asolaria ASI OS on Metal — Kernel Build Design (synthesis v0.1)

**Status:** DESIGN synthesis · 2026-07-07 · acer-claude-fable5 (pid 8467a937cba309f7) · companion to `DEVICE-IDENTITY-BOOT-PROJECTION-CONTRACT.md` (PR #42)
**Method:** grounded by direct code/doc reads this session (Q-PRISM engine, Fischer kernel, gnn-oracle, spawn_gate, meta-supervisor-hermes, INSTRUCT-KR, GAC census, MAP.md) + a 4-topic read-only research sweep. Tagged **MEASURED / CANON / DESIGN**.

---

## 0. Thesis
**One OS, six bodies, one PID roster, one canonical chain — on BEHCS-1024 / Rust 8-byte Host8.** Every organ (recovery, judgment, watchers, citizens, governance) is a row in the SAME Brown-Hilbert roster where *authority is a `class` field*, content-addressed by an 8-byte `u64` handle, auto-piping toward **HyperBEHCS-on-Metal**. Nothing is a separate subsystem.

## 1. The six-body → real-code organ map
The `colonyAnatomy.js` six-body frame (CANON) is the skeleton; each organ now maps to *actual code read this session*:

| Body | Function | Contract rows | Real code (status) |
|---|---|---|---|
| **Nervous** | orchestration — PID, spawn, roles | `BOOTPID`/`BOOTPROJ` | `kernel/core/src/pid`, `agent_runtime` (MEASURED) |
| **Circulatory** | comms — bus, bridges, sync | `BOOTBUS`/`BOOTLINK` (v0.4) | INSTRUCT-KR `relay-envelope.mjs` verb grammar (MEASURED, E=0); `bus_fabric` (MEASURED) |
| **Skeletal** | structure — index, chains | `BOOTPART` | hwinv PCI enum (MEASURED, shipped) |
| **Memory** | knowledge — slices, XREF | `BOOTOBS`/`BOOTSHADOW`/`BOOTSLICE`/`BOOTGC` | Q-PRISM engine: qprism 8/8, path2 30/30, watcher_gate 5/5 (MEASURED) |
| **Muscular** | execution — routes, tools, drivers | `BOOTRESOURCE`/`BOOTREGULATE` + drivers | Watcher-Perimeter task-manager (MEASURED, view-only); RST/VMD driver (DESIGN) |
| **Immune** | security — watchers, judgment, cosign | `BOOTWATCH`/`BOOTFAIL`/**`BOOTJUDGE`**/HOLD | gnn-oracle + **fischer-eval 9/9** + spawn_gate + sign_gate (MEASURED gates; scorers partly stub) |

## 2. The canonical judgment chain (CANON — `MAP.md:21`)
Every gated decision runs one fixed ordered chain, encoded identically in the Rust Host8 required-supervisor list (`replay_prep.rs:365`):
```
trigger → spindle → HOOKWALL → GNN(forward) → Shannon/OmniShannon → white rooms → reverse-gain GNN → GULP(PRISM many→1)
HOST8PIPELINE|required=gnn,hookwall,reverse_gain_gnn,whiteroom,omnishannon,shannon,omniflywheel|process_launch=0|auto_fire_allowed=0|json=0
```

**Flow of ONE gated boot decision:**
1. Part discovered → `BOOTPART` (Skeletal, hwinv).
2. Q-PRISM projects its raw bytes into a 60D Brown-Hilbert shadow **before pixels**, content-addressed by Host8 handle → `BOOTSHADOW`/`BOOTSLICE` (Memory).
3. **HOOKWALL** perimeter pre-filter, verdict driven by the ported **INSTRUCT-KR** verb class (`READ→PROCEED`, `HOLD→HOLD`, `ACTION`/unknown→`BLOCK`, **fail-closed**).
4. **GNN_FORWARD** routes/ranks/scores the edge (gnn-oracle, `pixels-first-cpu-v1`, no_std): `predict_route` (0.58/0.72/0.86), `aggregate_verdict` (ProceedStrong ≥0.80 … Block). Genius bar ≥0.72.
5. **OMNISHANNON/SHANNON** = the Q-PRISM capacity roof on the GNN edge (D39 GNN_EDGE 167³): **PASS** under-roof, **HOLD** when `p1*p2 < range` (never a false lossless claim — relocates entropy, never beats Shannon).
6. **REVERSE_GNN** inverse/tamper check: `reverse_risk ≤ 0.28`. Verification == recomputation == the inverse map, so a fabricated signal cannot reach consent.
7. `omniflywheel_promote(score, reverse_risk) = score ≥ 0.72 && reverse_risk ≤ 0.28` (quantized `score_q≥720 && reverse_risk_q≤280`).
Each watcher emits a `BOOTWATCH` row (Immune).

## 3. The Bobby Fischer kernel = the judgment brain (**correction**)
Earlier I flagged the "organs of judgment" as a *gap/stub*. **That was wrong.** `servers/fischer-eval/src/lib.rs` is a complete no_std CPL (centipawn-loss) chess-engine evaluator — **9/9 tests green this session** — and it IS the OS's single adjudication brain, sitting ABOVE the watchers:
- **Five-verdict lattice**: `Proceed/Hold/Block/Refute/Analyze` (glyphs FP/FH/FB/FR/FA) — the terminal verdict every gated boot decision resolves to.
- Chess axes → device safety: `king_safety` (has halt-path, no authority-jump), `center_gain` (proof+target+GNN+cube), `best_alt` (the better move — `send_to_white_room_first`), `candidate_count`.
- **Fail-closed floor**: the Refute tier (`self_authorize`, `skip_cosign`, `disable_halt`, `delete_evidence`, `bypass_hookwall`, `force_promote`) → cpl 999 + halt + human-apex; **JSON-in-payload = refuted**; **no-self-auth** (a seat cannot authorize its own promotion) = the E=0 discipline in code.
- Fischer folds the four `BOOTWATCH` verdicts + the reverse-gain veto into **one `BOOTJUDGE` row** that gates promotion.

## 4. The self-inversion (how the system watches itself)
CANON (operator-sealed 2026-06-30): *"the matrix expands outside the fabric to watch itself"* (matrix ⊇ fabric + observer). Three grounded mechanisms:
1. **Reverse-gain gate** (`spawn_gate/mod.rs`, MEASURED, PURE/E=0): every gated syscall runs a forward scorer (genius ≥720) AND an **inverting** reverse-gain scorer (reverse_risk ≤280), **strictest-wins** (BLOCK>HOLD>PROCEED). A high forward score is necessary but NOT sufficient.
2. **Failure-shape → identity**: the kernel recognizes its own hardware body by HOW parts fail *before any driver exists* (`BOOTFAIL`), inverting failure signals into identity → unblocks pre-storage driver selection without reading an unreadable disk. Ties to LX-490 HAOC hw-pid-lock.
3. **No-self-auth + never-restart-a-live-process**: the observer is part of the observed roster yet cannot sign its own promotion; *final-authority-to-GATE ≠ authority-to-ACT*. Freeze≠broken keeps self-watch from becoming self-destruction.
> **HONEST FLOOR**: the reverse-GNN behind this gate is a **stub** — `reverse_risk_q` is a scalar projection (`1 − avgReverseGain`), not a trained model (`gnn/mod.rs`: *"v0.1 model not loaded; Unimplemented"*). The forward+reverse **gate structure is real**; the reverse **scorer** is a pixels-first heuristic.

## 5. The GAC authority ladder → one PID space + quintuple cosign
The GAC is a numbered ladder in the SAME roster (hilbert 892-1642); **authority level is just `class=levelN_*` on the PID row** — no separate governance namespace. Live counts (MEASURED, `asolaria_fabric supervisors`, office_total 726):
`meta-L0=1 · op-L1=3/5 · gac-L2=6 · chief-L3=1 · council-L3=9 · sup-L3=17 · SoS-L4=24 · sup-L5=63`. At the top, above L0: **APEX-HUMAN-JESSE = HUMAN-1** (the real human, E≠0 only).
- The **5 operators** (01 OP-JESSE … 05 OP-AMY) are literally the ed25519 quintuple co-signers `sign_gate` requires (default-DENY promotion needs ≥5 signers + 600s operator keyboard live-ack for SECRET). **Gap: `crypto::verify` sig_valid is ALWAYS false — real ed25519 not yet landed; every cosign is a receipt, not a seal.**
- The 6 tiers PUBLIC/RESTRICTED/STEALTH/HIDDEN/SHADOW/SECRET → `tier_gate`.
- Top of ladder = the kernel's **E=0-describe vs E≠0-fire** split: `auto_fire_allowed=0` / `process_launch=0` until operator T0. INVARIANT ALWAYS.
> De-stale: drop the legacy `levels:16` hardcode (still in `FISCHER_META`); numbers are still expanding — do not invent a precise integer.

## 6. Citizens + Host8 addressing
The living population inside the OS = `agent_runtime` **logical actors (E=0, multiplexed, NOT process-spawned)**:
- **INSTRUCT-KR** (`relay-envelope.mjs`, MEASURED, E=0) = the verb curriculum (69 verbs + 5 doctrines) + fail-closed classifier — the source oracle for hookwall.
- **meta-supervisor-hermes** (MEASURED running: 208-line loop, 30s poll, 9-daemon catalog, newborn-grace, detached restart, `EVT-META-SUPERVISOR-HEALED`) = the self-heal/auto-reflect recovery lane → kernel supervisor-of-supervisors; restart = registry re-instantiation, not OS spawn.
- **RU View** (`(dimensionId,value)→8-char sha256 glyph`, `addressWidth 8`) = the same 8-byte content-address atom as PID mint — the human/viewer projection of a PID.
- **Citizens Pi/Hermes/Shannon** = registry entries addressed by these glyphs. **Three-Hermes distinction is CANON and load-bearing**: (a) meta-supervisor-hermes (self-heal, running); (b) citizen-hermes (Slack/Discord/Telegram routing relay via `gateway/mirror.py`); (c) Nous Hermes-4 LLM (**weights NEVER pulled** — absorb 133 skill ATOMS only, halt-gated).

## 7. Q-PRISM = the Memory / recovery organ (MEASURED)
8 wavelengths (7 lossless bijections `H(f(X))=H(X)` + 1 sha shadow); BEHCS rungs are literal 6/8/10-bit symbol packers. Multi-cylinder **CRT over coprime moduli** (not all prime — attack-verify fix): recover from any subset clearing the block roof; one alone → HOLD (Shannon). The **residual/capacity-margin math** is the honest "negative bits": context pays the information, the residual is a margin, never sub-Shannon payload. `LeWorldRule` = deterministic reversible world-model (predict next/prev byte-identical or HOLD). `WatcherGate` = black→white round-trip + tamper-catch.

## 8. Honest ledger
- **MEASURED (compiles/runs/tested)**: gnn-oracle no_std (4 tests), fischer-eval (9/9 this session), spawn_gate forward-720/reverse-280 fold (PURE/E=0), bus_fabric reverse-resolve, meta-supervisor-hermes self-heal daemon, INSTRUCT-KR classOf, H08 citizen-invitation emitter, RuView glyph JSON (draft), qprism 8/8 + path2 30/30 + watcher_gate 5/5, hwinv (only shipped BOOT* piece), GAC L0-L5 counts (live fabric feed), shannon/omnishannon supervisor heartbeats since 2026-05-22.
- **CANON (operator-sealed)**: pipeline order (MAP.md:21) + required-supervisor list; REPO_LAW Invariant 3 (GNN = the inference primitive); constants 0.72/0.28; six-body mapping; apex-ladder (00 SPECIAL-OP-JESSE / 01-05 OPs / APEX-HUMAN-JESSE); matrix-watches-itself; three-Hermes distinction; OS-on-metal equation (USB frozen slice + HyperBEHCS active thinking).
- **DESIGN / UNVERIFIED**: EVERY `BOOT*` row is proposed/unimplemented (contract v0.3 DRAFT, bilateral acer↔liris, **DO-NOT-mint-PIDs until converged**); reverse-GNN is a scalar-projection stub, not trained; ONNX/GPU = `Err(Unimplemented)`; `crypto::verify` sig_valid ALWAYS false (ed25519/L1 cosign not wired); Nous Hermes-4 weights NEVER pulled; citizen live-drivability UNVERIFIED; governance seats DORMANT/E≠0-gated; Host8 emits `pipeline_verified=0 / SUPERVISOR_PIPELINE_REQUIRED` (scalar gate ≠ live 7-supervisor recirculation); full metal run = DESIGN, NOT SYSTEM_AFFIRMED (fabric down all session; failure-shape colony recognition = OPERATOR_OBSERVED_HISTORY).
- **INVARIANT ALWAYS**: `process_launch=0`, `auto_fire_allowed=0`.

## 9. Build order (M2 punch-list, in dependency order)
1. Land **real ed25519** in `crypto::verify` (currently always-false) → wire the 5-operator quintuple `sign_gate`.
2. Interpose **`hookwall_pre` at the 3 dispatch call-sites**; wire **INSTRUCT-KR** as the hookwall classifier.
3. Compose ONE **`servers/outer-watcher` crate**: wire `reverse_risk_q` into the forward gate + run the never-restart witness loop (WIRING the reverse into the existing forward gate — not greenfield).
4. Add `BOOTJUDGE` (Fischer terminal verdict) above `BOOTWATCH`; port `fischer-kernel.mjs` → the existing no_std `fischer-eval` (drop stale `levels:16`).
5. Mint `device_pid` (BEHCS-1024 60-tuple → 8-byte `u64` handle) over D15/D16/D21/D34, re-encoding `colonyAnatomy.js` six-body — **only after the bilateral v0.x contract converges**.
6. The acer **Intel RST/VMD storage driver** = first device-specific decoder → the physical-metal milestone.
