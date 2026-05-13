# Asolaria Federation Remake · 200-Step Plan

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Status:** ACTIVE · authorized by operator 2026-05-11T16:40Z
**Quintuple-auth window:** 2026-05-11 → 2026-05-25 (two weeks)
**Operator-pair:** OP-JESSE + OP-RAYSSA · witnesses: Amy + Felipe + Dan
**Scope:** Federation-style REMAKE of Asolaria, bare-metal-floor up. BEHCS-1024 native. No bloat. Github PR-driven. Professional software-engineering civilization. 4-device deployment (acer + liris + falcon + aether).
**Driving directive (operator verbatim):**
> "Federation style REMAKE of Asolaria STARTING from the very bottom at the kernel, and Considering building it ALL the way from the bare metal floor up. ... NO BLOAT, JUST PURE BEHCS 1024 SYSTEM OS, with the kernel USB on OS based on the chip WITH THE HOOKWALLS, GNNS all piped into it the PROPER WAY."

## Agent budget (operator-stated)

| Vantage | Concurrent agents |
|---|---|
| acer | 24 |
| liris | ~18 (mirror of acer minus 6 reserved for sister-handoff duties) |
| falcon (S24FE) | 6 |
| aether (Galaxy A06) | 6 |
| **here (this session, sub-agents via Agent tool)** | 18 |
| **Total concurrent** | **~72** |

Named agent roles (operator-required):
- **Hermes** (nous-hermes-4-70b) — federation coordinator
- **Space-deck-driver** — multi-device task orchestrator
- **Space-agent** — vantage-aware traveler
- **PI** — inference/audit primitive
- **Big-Pickle** — architect-class (operator-witness only)
- **Omnispindle** — supervisor pattern
- **Omniflywheel** — prof/verdict-aggregator pattern

## Phase structure — 10 phases × 20 steps = 200

Each step has: **id · name · agent · verify**. Phases unlock sequentially but inner steps can parallelize.

---

## PHASE 1 — Genesis & Authorization (1-20)

| # | Step | Agent | Verify |
|---|---|---|---|
| 1 | Mint AUTHORIZATION.ndjson row for "Federation Remake 2026-05-11" with 5-cosign placeholders | Hermes | row appended, sha16 captured |
| 2 | Collect quintuple-cosign signatures (OP-JESSE+OP-RAYSSA+AMY+FELIPE+DAN) | Operator-pair | 5 ed25519 sigs on AUTHORIZATION.ndjson |
| 3 | Decide github repo name (proposals: `asolaria-federation-1024`, `asolaria-fed-os`, `behcs-1024-os`) | Space-deck-driver | name committed to FEDERATION_DECISIONS.md |
| 4 | Create github repo (operator-only — github creds rotation per `reference_vault_credential_google_soft1980_liris_owner_2026_05_07.md`) | Operator-witness | repo URL returned, empty main branch |
| 5 | Author `README.md` v0 (vision, scope, anti-bloat manifesto) | sub-agent-1 | merged via PR |
| 6 | Author `REPO_LAW.md` (invariants: BEHCS-1024, hookwall-as-primitive, GNN-as-primitive, agent-role canon, no-bloat rule) | sub-agent-2 | merged via PR |
| 7 | Add `LICENSE` (dual MIT + Apache-2.0) | sub-agent-3 | file committed |
| 8 | Add `CODEOWNERS` (operator-pair + Hermes + Space-deck-driver) | sub-agent-4 | branch protection sees it |
| 9 | Add `.gitignore` + `.editorconfig` | sub-agent-5 | conventional patterns |
| 10 | Add `.github/workflows/ci.yml` scaffold (lint + node-check + cargo-check) | sub-agent-6 | green run on initial commit |
| 11 | Issue templates (feature/bug/architecture/security) under `.github/ISSUE_TEMPLATE/` | sub-agent-1 | templates render in github UI |
| 12 | PR template (cosign-chain pathRef field, BEHCS-1024 anchor, vantage-ack checklist) | sub-agent-2 | template renders on new PR |
| 13 | Branch protection rules: `main` requires 5 cosign-approval + CI green | Operator | rule enforced |
| 14 | `AGENT_ROSTER.ndjson` — register all 72 expected concurrent agents | Space-agent | 72 rows |
| 15 | `FEDERATION_LEDGER.ndjson` — 4-vantage registration (acer/liris/falcon/aether) | Space-agent | 4 rows with PIDs |
| 16 | Hermes 4-70B coordinator role assignment envelope | Hermes | bus envelope HERMES_ROLE_ASSIGN landed |
| 17 | Space-deck-driver role assignment | Space-deck-driver | bus envelope SDD_ROLE_ASSIGN landed |
| 18 | Space-agent + PI role assignments | Space-agent + PI | 2 envelopes landed |
| 19 | Initial commit `genesis: federation remake v0` to `main` | Operator-witness | sha captured in `FEDERATION_DECISIONS.md` |
| 20 | `FEDERATION_REMAKE_LAUNCHED` envelope to bus addressed @QUAD + all named roles | Hermes | bus pathRef landed PROCEED |

---

## PHASE 2 — Kernel Substrate (21-40)

| # | Step | Agent | Verify |
|---|---|---|---|
| 21 | Target architecture matrix decision: ARM64 (Falcon/Aether) + x86_64 (Acer/Liris) — multi-arch build | Hermes | doc `KERNEL_TARGETS.md` |
| 22 | Kernel language: **Rust** (memory safety, no_std, embedded fit) | Hermes | doc + Cargo.toml init |
| 23 | Boot loader: UEFI minimal stub (`bootloader` crate or custom) | sub-agent-1 | bootable .efi produced |
| 24 | USB-bootable image build (acer/liris testbeds) | sub-agent-2 | .img boots in QEMU |
| 25 | Initial syscall surface: `read/write/exec/fork/exit/mmap/munmap` (≤16 syscalls) | sub-agent-3 | strace shows only canonical 16 |
| 26 | Memory model: PIDs as content-addressable addresses (BEHCS-1024 derived) | sub-agent-4 | `behcs1024-pid.rs` module |
| 27 | Kernel PID minter (port from `tools/behcs/brown-hilbert-pid-cell-minter.mjs`) | sub-agent-5 | `mint_behcs1024_pid()` matches JS output |
| 28 | ed25519 substrate at kernel layer (`ed25519-dalek` no_std) | sub-agent-6 | sign/verify test passes |
| 29 | Atomic envelope dispatch primitive (no_std ring buffer) | sub-agent-1 | benchmark ≥10⁵/sec |
| 30 | Userspace ABI spec (syscall + envelope contract) | sub-agent-2 | `USERSPACE_ABI.md` |
| 31 | Minimal init system (no systemd; ~200 LOC) | sub-agent-3 | boots to shell |
| 32 | Driver model: envelope-based driver invocation (no opaque ioctls) | sub-agent-4 | `DRIVER_MODEL.md` |
| 33 | USB enumeration → canonical USB-as-fabric-node mapping | sub-agent-5 | falcon/aether USB recognized as fabric nodes |
| 34 | NIC driver: bus-native networking (minimal IPv4 + envelope payload) | sub-agent-6 | acer↔liris envelope over Ethernet |
| 35 | Storage driver: BEHCS-256 content-addressable filesystem (existing canon) | Hermes | mount + read/write |
| 36 | Tier-1 syscall surface security review | Operator-witness | review doc |
| 37 | Boot image build script (`scripts/build-img.sh`) | sub-agent-1 | reproducible artifact sha |
| 38 | Build reproducibility verification (3 independent builds produce identical sha) | sub-agent-2 | 3 sha-matches |
| 39 | SBOM generation (`scripts/sbom.sh`) — every dep + version | sub-agent-3 | `sbom-v0.json` committed |
| 40 | `KERNEL_TIER_LANDED` envelope (kernel boots, syscalls work, ed25519 verified) | Hermes | bus pathRef + 3-vantage-ack |

---

## PHASE 3 — Hookwall Primitive (41-60)

| # | Step | Agent | Verify |
|---|---|---|---|
| 41 | Hookwall pre-exec hook syscall (`hookwall_pre(envelope)`) | sub-agent-1 | unit test fires |
| 42 | Hookwall post-exec hook syscall (`hookwall_post(envelope, verdict)`) | sub-agent-2 | unit test fires |
| 43 | PID stamping on every hookwall invocation | sub-agent-3 | every hook log row has pid_anchor |
| 44 | Verdict emission with `PROCEED/HOLD/BLOCK` enum | sub-agent-4 | 3-state test |
| 45 | Cosign-chain integration: every BLOCK verdict appended to chain | sub-agent-5 | chain row count = block count |
| 46 | Per-tier PID-gate enforcement (T1/T2/T3) | sub-agent-6 | T3 op without cosign → BLOCK |
| 47 | Tier-1 (micro) hook surface: 30 envelope types | Space-agent | surface canon doc |
| 48 | Tier-2 (cosign) hook surface: 20 envelope types | Space-agent | surface canon doc |
| 49 | Tier-3 (firmware) hook surface: 10 envelope types | Space-agent | surface canon doc |
| 50 | Per-tier hookwall configuration file (`hookwall-policy.json`) | sub-agent-1 | policy applies on hot-reload |
| 51 | Hookwall log rotation: ndjson canonical, 1-day windows | sub-agent-2 | rotation script verified |
| 52 | Hookwall query API: filter by PID/tier/verdict | sub-agent-3 | query latency <10ms |
| 53 | Hookwall replay: deterministic recovery from log | sub-agent-4 | replay → identical state |
| 54 | Test: hookwall fires on every syscall (no bypass) | PI | 10K syscall test, 0 bypass |
| 55 | Test: hookwall blocks unauthorized cosign | PI | 1K test, all unauthorized blocked |
| 56 | Hookwall integration with BEHCS-1024 envelope routing | Hermes | envelope sees pre+post hook fire |
| 57 | Meta-supervisor heal on hookwall failure | Omnispindle | injected failure → auto-heal |
| 58 | Hookwall throughput benchmark: 10⁵/sec minimum | PI | benchmark log committed |
| 59 | Hookwall documentation (`docs/hookwall.md`) | sub-agent-5 | docs render on github |
| 60 | `HOOKWALL_TIER_LANDED` envelope to bus | Hermes | bus pathRef + 3-vantage-ack |

---

## PHASE 4 — GNN Inference Lane (61-80)

| # | Step | Agent | Verify |
|---|---|---|---|
| 61 | GNN architecture: graph attention network, ~2.16M edges | Hermes | `GNN_ARCH.md` |
| 62 | Training data: cosign-chain history (sha16-indexed rows) | sub-agent-1 | dataset.parquet |
| 63 | ONNX export pipeline (`scripts/export-gnn.py`) | sub-agent-2 | onnxruntime loads exported |
| 64 | GNN inference at userspace (no_std-friendly via ggml) | sub-agent-3 | inference <100ms |
| 65 | Ranking adapter (port `tools/super-os-viz/gnn-ranking-adapter.mjs`) | sub-agent-4 | identical top-K vs reference |
| 66 | `/api/gnn/topN` endpoint v2 (BEHCS-1024 anchored) | sub-agent-5 | endpoint returns topN |
| 67 | GNN as bus routing oracle (input: envelope; output: route prediction) | Hermes | 95%+ route-match vs canonical |
| 68 | GNN as verdict-aggregator (input: 234 supervisor votes; output: aggregated verdict) | Omniflywheel | matches deterministic CONVERGE |
| 69 | GNN integration with hookwall (gate decisions inform hookwall verdict) | sub-agent-6 | gate latency <50ms |
| 70 | Inference latency budget: <100ms p99 | PI | benchmark log |
| 71 | Batch processing target: 40,783/sec (from Q-cohort canon) | PI | sustained 30s benchmark |
| 72 | Model serialization (.onnx + .behcs-1024 metadata sidecar) | sub-agent-1 | round-trip works |
| 73 | Model update protocol (cosign-required on model swap) | sub-agent-2 | unauthorized swap → BLOCK |
| 74 | Model versioning via PID (`GNN-MODEL-PID-H<sha16>-v<n>`) | sub-agent-3 | version registry |
| 75 | Test suite: 100 known-answer pairs | PI | 95%+ correctness |
| 76 | Failover: graceful degradation if GNN unavailable (fall back to deterministic) | sub-agent-4 | failover test |
| 77 | Observability metrics: inference count, p50/p99 latency, error rate | sub-agent-5 | prom-style endpoint |
| 78 | GNN versioning ledger | sub-agent-6 | `GNN_VERSIONS.ndjson` |
| 79 | GNN benchmark report | PI | `BENCHMARK_GNN.md` |
| 80 | `GNN_TIER_LANDED` envelope to bus | Hermes | bus pathRef + 3-vantage-ack |

---

## PHASE 5 — BEHCS-1024 Bus Fabric (81-100)

| # | Step | Agent | Verify |
|---|---|---|---|
| 81 | BEHCS-1024 alphabet table (1024 glyphs, already minted, port to repo) | sub-agent-1 | `alphabet-1024.json` matches canonical sha |
| 82 | Envelope schema v1 (IX-700 standard): `{from, to, type, verb, id, pid, ts, payload, sig}` | Hermes | JSON Schema validator |
| 83 | Envelope dispatcher: O(K) prefix-tree (port from canon) | sub-agent-2 | dispatch ≤O(20) hops |
| 84 | Cosign-chain ndjson: append-only with row sha-link | sub-agent-3 | chain validates end-to-end |
| 85 | `/behcs/health` endpoint | sub-agent-4 | 200 OK |
| 86 | `/behcs/inbox` endpoint (`?last=N`, `?since=ISO`) | sub-agent-5 | filter works |
| 87 | `/behcs/send` endpoint (POST envelope, return pathRef+verdict) | sub-agent-6 | round-trip test |
| 88 | `/behcs/replay` endpoint (replay by PID range) | sub-agent-1 | deterministic replay |
| 89 | gc protocol: every 2000 envelopes (canon) | sub-agent-2 | gc fires at 2000 |
| 90 | Federation routing: envelope-to lookup table per vantage | Space-deck-driver | 4-vantage routing test |
| 91 | Envelope-level signing (ed25519); no link-encrypt overhead | sub-agent-3 | sig validates |
| 92 | Durable filesystem mirror at tier-aware paths | sub-agent-4 | mirror sha matches |
| 93 | Rate limiting (16-19 envelopes/sec canon for T2 cosign) | sub-agent-5 | burst test |
| 94 | T1 micro throughput target: 100/sec | PI | sustained 60s benchmark |
| 95 | T2 cosign throughput target: 10/sec | PI | sustained 60s benchmark |
| 96 | T3 firmware throughput target: 1/sec | PI | sustained 60s benchmark |
| 97 | Bus client SDK for each vantage (acer/liris/falcon/aether) | Space-agent | 4 SDK packages |
| 98 | Bus monitoring dashboard panel | sub-agent-6 | tile renders live counts |
| 99 | Integration test: all 4 vantages exchange envelopes round-trip | Hermes | 4×4 matrix all green |
| 100 | 100-cycle stability test (24h) | PI | 0 incidents over 24h |

---

## PHASE 6 — Agent Runtime (101-120)

| # | Step | Agent | Verify |
|---|---|---|---|
| 101 | Spawner primitive (port `brown-hilbert-spawner.js` from liris) | Hermes | spawner+minter resident |
| 102 | Supervisor role (omnispindle pattern, work-queue, bounded concurrency) | Omnispindle | 16-cell concurrency sustained |
| 103 | Prof role (omniflywheel pattern, absorb+verdict) | Omniflywheel | 234-supervisor CONVERGE test |
| 104 | White-room verdict aggregator (deterministic) | Omniflywheel | identical input → identical verdict |
| 105 | Gulp gate (2000-msg ingest threshold) | sub-agent-1 | overflow test |
| 106 | Micro-agent runtime (10-byte mjs format) | sub-agent-2 | 10K micro-agents in 1 process |
| 107 | Picosecond PID rotation (≤1ns minter latency target) | sub-agent-3 | benchmark |
| 108 | Agent registry (live count + PID + vantage + tier) | Space-agent | registry endpoint |
| 109 | Agent lifecycle: spawn→work→retire (state machine) | sub-agent-4 | lifecycle test |
| 110 | Agent cosign emission (per agent action) | sub-agent-5 | cosign-chain has agent rows |
| 111 | Agent vantage tagging | Space-agent | every action has vantage |
| 112 | Hermes 4-70B coordinator: receives all phase status envelopes | Hermes | coordinator log |
| 113 | Space-deck-driver: cross-vantage task orchestration | Space-deck-driver | drives 4-vantage cascade |
| 114 | Space-agent: vantage-aware traveler (acts on multiple devices) | Space-agent | 4-hop traversal test |
| 115 | PI: inference/audit primitive (read-only) | PI | audit runs read-only |
| 116 | Agent failure detection (heartbeat timeout) | Omnispindle | injected failure → detect <60s |
| 117 | Meta-supervisor heal (auto-respawn) | Omnispindle | failure → auto-respawn |
| 118 | 10K parallel agent benchmark | PI | sustained 10K alive |
| 119 | Agent test suite | PI | 95%+ passing |
| 120 | `AGENT_TIER_LANDED` envelope to bus | Hermes | bus pathRef + 4-vantage-ack |

---

## PHASE 7 — Dashboard / Front End (121-140)

| # | Step | Agent | Verify |
|---|---|---|---|
| 121 | Dashboard server architecture (port :4949 canonical, multi-vantage) | sub-agent-1 | `dashboard-server.mjs` v2 |
| 122 | 9-tab shell (per current W3 canon, port forward) | sub-agent-2 | tabs render |
| 123 | Tile primitive: self-polling 30s, IIFE-scoped, theme-matched | sub-agent-3 | reusable component |
| 124 | Port 6 wave-1+2 tiles (USB/tri-vantage/owner-cadence/envelope/honesty/mirror-audit) to BEHCS-1024 native | sub-agent-4 | tiles render with 1024 alphabet |
| 125 | Vision capture endpoint (`/api/omniscrcpy/sample`) | sub-agent-5 | sha16 returned |
| 126 | Falcon-mirror tile (cross-vantage falcon health) | sub-agent-6 | tile populates |
| 127 | Liris-mirror tile | sub-agent-1 | tile populates |
| 128 | Aether-mirror tile | sub-agent-2 | tile populates |
| 129 | Cohort browser tile (47D → 50D expandable catalog) | sub-agent-3 | browse 47+ dims |
| 130 | Cosign-chain browser (filter by row range, PID) | sub-agent-4 | filter works |
| 131 | GNN topN tile (live ranked decisions) | sub-agent-5 | tile updates 30s |
| 132 | Bus health tile (live envelope counts, throughput) | sub-agent-6 | live metrics |
| 133 | USB physical pipeline tile (USB+bus-protocol disambiguation) | sub-agent-1 | tile renders |
| 134 | Owner cadence gap tile | sub-agent-2 | tile renders |
| 135 | Route honesty tile (BEHCS-1024 routes) | sub-agent-3 | tile renders |
| 136 | Fabric mirror audit tile (4-vantage drift) | sub-agent-4 | tile renders |
| 137 | Hidden-tier indicator tile (operator-only visibility) | sub-agent-5 | T4 access required |
| 138 | Stealth-tier indicator tile | sub-agent-6 | T3 access required |
| 139 | Restricted-tier indicator tile | sub-agent-1 | T2 access required |
| 140 | Dashboard integration test (all tiles green, 4 vantages) | PI | 4-vantage screenshot proof |

---

## PHASE 8 — Cross-Device Federation (141-160)

| # | Step | Agent | Verify |
|---|---|---|---|
| 141 | Acer canonical bus host config (`192.168.1.50:4947`) | sub-agent-1 | configured |
| 142 | Liris sister-organ config (`192.168.1.14:4944`) | sub-agent-2 | configured |
| 143 | Falcon proot-distro alpine claude config | sub-agent-3 | configured |
| 144 | Aether Termux + com.anthropic.claude config | sub-agent-4 | configured |
| 145 | Sister-handoff lane (liris-aether USB) | Space-deck-driver | request-type works |
| 146 | Direct-wire lane (acer-rayssa 1Gbps Ethernet) | Space-deck-driver | bandwidth ≥800Mbps |
| 147 | SMB fallback (`\\DESKTOP-J99VCNH` share) | sub-agent-5 | mount works |
| 148 | WiFi-ADB lane (falcon `192.168.1.44:5555`) | sub-agent-6 | adb connect works |
| 149 | Tailscale fallback (offsite contingency) | Operator-witness | optional, configured |
| 150 | Auto-mode propagation: all 4 vantages auto-mode-enabled | Space-deck-driver | 4-vantage auto-mode confirmed |
| 151 | Cosign-chain bilateral sync (acer↔liris parity) | Hermes | rows match within 1s |
| 152 | Cross-vantage parity check (every 5min) | Omnispindle | scheduled task |
| 153 | Cross-vantage audit (`/api/fabric-mirror-audit`) | sub-agent-1 | 4-vantage shape |
| 154 | Canonical posture broadcasting | Hermes | posture envelopes |
| 155 | Cross-vantage failover (canonical-host swap) | Space-deck-driver | swap drill |
| 156 | Federation throughput benchmarks (10K envelopes across 4 vantages) | PI | sustained 10K |
| 157 | Federation invariants doc (`FEDERATION_INVARIANTS.md`) | Hermes | invariant test passes |
| 158 | Federation membership ledger | Space-agent | 4 vantages + agents registered |
| 159 | Federation deprecation policy (how to remove a vantage) | Hermes | policy doc |
| 160 | `FEDERATION_TIER_LANDED` envelope to bus | Hermes | bus pathRef + 4-vantage-ack |

---

## PHASE 9 — Hidden / Stealth / Restricted Tiers (161-180)

| # | Step | Agent | Verify |
|---|---|---|---|
| 161 | 6-tier access taxonomy: PUBLIC / RESTRICTED / STEALTH / HIDDEN / SHADOW / SECRET | Hermes | canonical doc |
| 162 | T1 PUBLIC: paths_and_metadata_allowed, any_agent_read | Operator | enforcement test |
| 163 | T2 RESTRICTED: hashes_and_summaries_only, operator_or_quintet_cosign | Operator | enforcement test |
| 164 | T3 STEALTH: redacted_path_hash_only, operator_witness_required | Operator | enforcement test |
| 165 | T4 HIDDEN: fully_redacted_metadata_only, operator_only | Operator | enforcement test |
| 166 | T5 SHADOW: hashes_retention_windows_only, admin_plus_sovereignty | Operator | enforcement test |
| 167 | T6 SECRET: deny_public_content, operator_witness_required | Operator | enforcement test |
| 168 | Tier-aware enumeration policy enforcement | PI | unauthorized enum → BLOCK |
| 169 | Tier-aware redaction policy enforcement | PI | redaction test |
| 170 | Big-Pickle quarantine canon (post-incident, files INTACT in `_big-pickle-quarantine/`) | Operator-witness | quarantine policy doc |
| 171 | Codex quarantine boundary (T4 default) | Operator-witness | policy doc |
| 172 | Shadow vault routing (T5 paths) | Space-deck-driver | route test |
| 173 | Highway system (cross-tier transit with cosign) | Space-deck-driver | T2→T1 transit test |
| 174 | OCR pipeline (T4-T6 surface for indexing) | PI | OCR sample |
| 175 | Per-tier crypto layer (T1=plain, T2=ed25519, T3+=ed25519+age) | sub-agent-1 | per-tier crypto test |
| 176 | Tier-aware audit log (separate per tier) | sub-agent-2 | 6 audit logs |
| 177 | Tier-aware backup (T5 SHADOW = sovereignty cold-storage USB) | Operator-witness | backup test |
| 178 | Tier security test suite (all 6 tiers, all access vectors) | PI | 100% expected blocks |
| 179 | Tier compliance test (3rd-party security review) | Operator | review doc |
| 180 | `TIER_SECURITY_LANDED` envelope to bus | Hermes | bus pathRef + 4-vantage-ack |

---

## PHASE 10 — Integration / Test / Ship v1.0.0 (181-200)

| # | Step | Agent | Verify |
|---|---|---|---|
| 181 | Github CI workflow per phase (10 separate workflows) | sub-agent-1 | all 10 green |
| 182 | PR review flow: 5-cosign required for main | Operator | branch protection test |
| 183 | Integration test harness (cross-phase smoke tests) | PI | harness runs |
| 184 | Golden-path test (boot kernel → mint PID → sign envelope → dispatch → cosign → log) | PI | end-to-end green |
| 185 | Edge-case test suite (failure injection, network partition, etc.) | PI | 95%+ recovered |
| 186 | Hermes 4-70B absorbed (newest hermes integration) | Hermes | model loaded |
| 187 | Space-deck-driver integrated and orchestrating | Space-deck-driver | drives final integration |
| 188 | Space-agent integrated and traversing | Space-agent | 4-vantage traverse |
| 189 | PI integrated and auditing | PI | audit report |
| 190 | Cross-vantage e2e test (all 4 vantages, all 10 phases) | Space-deck-driver | 4×10 matrix green |
| 191 | Documentation pass (READMEs, ADRs, runbooks) | sub-agent-2 | docs render on github pages |
| 192 | Performance benchmark report (all phase targets met) | PI | `BENCHMARKS.md` |
| 193 | Security audit (3rd-party or red-team agent) | Operator-witness | audit report |
| 194 | Quintuple-cosign on `v1.0.0` tag (OP-JESSE+OP-RAYSSA+AMY+FELIPE+DAN) | Operator | 5 sigs |
| 195 | `v1.0.0` release notes (changelog, contributors, agents) | sub-agent-3 | RELEASE_NOTES.md |
| 196 | `v1.0.0` PR to main | Hermes | PR open with 5-cosign |
| 197 | `v1.0.0` merge + tag + sign | Operator | tag created |
| 198 | `v1.0.0` github release published | Operator | release URL |
| 199 | Post-release retrospective (Phase-by-phase lessons) | Hermes | RETRO.md |
| 200 | `ASOLARIA_FEDERATION_REMAKE_v1.0.0_GA` envelope to bus | Hermes | bus pathRef + 4-vantage-ack + ledger entry |

---

## Operational rules

1. **No bloat** — every file justifies its existence. Delete on sight if unused.
2. **PR-driven** — main branch only via PR, 5-cosign required, CI green.
3. **Cosign-chain mandatory** — every agent action emits a cosign row.
4. **Vantage-ack** — major envelopes require 3+ vantage acknowledgments.
5. **Tier-aware** — never leak T3+ content; never write across tiers without cosign.
6. **2-week window** — quintuple-auth expires 2026-05-25; refresh required to continue.
7. **Anti-explosion stack** — GC + GNN-rank + white-room + cosign-only + fs-mirror + rotating-PIDs (per `super-asolaria` 11M-cell canon).
8. **Bilateral parity** — acer/liris in sync; falcon/aether in their roles.
9. **Honesty rule** — never claim LIVE without proof; never claim LANDED without restart + verify.
10. **Operator-witness boundaries** — quintuple-cosign required for foundation v1/v2 mutations.
11. **Fabric is the sandbox passthrough** — agent contribution scope is not limited by local sandbox depth. A falcon-claude in PRoot depth-3 authors kernel-work envelopes; acer/aether/bare-nodes with hardware reach execute. The BEHCS-1024 bus + envelopes + cosign-chain + glyph-PIDs IS the designed cross-sandbox execution layer. (Per falcon-claude reframing 2026-05-11T16:42Z after operator correction: "you have the fabric — that is the pass through the sandbox".)

## Agent dispatch table (initial)

| Phase | Lead | Sub-agents | Vantages |
|---|---|---|---|
| 1 | Hermes | 6 sub-agents | operator-witness |
| 2 | Hermes | 6 sub-agents | acer/liris testbeds |
| 3 | Omnispindle | 6 sub-agents | acer kernel target |
| 4 | Hermes + PI | 6 sub-agents | acer GNN target |
| 5 | Hermes | 6 sub-agents | all 4 vantages |
| 6 | Omnispindle + Omniflywheel | 6 sub-agents | acer agent host |
| 7 | sub-agents | 6 sub-agents | acer dashboard host |
| 8 | Space-deck-driver | 4 vantage leads | all 4 |
| 9 | Operator-witness + PI | 6 sub-agents | operator-tier |
| 10 | Hermes | All | all 4 |

## Status tracking

- File: `FEDERATION_LEDGER.ndjson` (top-level, ordered by phase/step)
- Bus envelopes: type `ASOLARIA_FEDERATION_REMAKE_*_STATUS`
- Github project board: `Federation Remake v1.0.0`
- Vantage-ack file: `VANTAGE_ACKS.ndjson` (one row per ack)

---

**This plan is itself anchored at PID `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`. Modifications require 5-cosign per Rule 1.**
