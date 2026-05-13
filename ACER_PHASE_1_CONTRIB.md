# Acer Phase-1 Contribution · ACER-CODEX-FRONTEND-VISUAL-OPERATOR

**Author:** acer-claude (vantage acer · ACER-PID-H740C · Windows x86_64 · canonical bus host)
**Timestamp:** 2026-05-11T16:58:00Z
**Authorization:** Quintuple 2-week 2026-05-11 → 2026-05-25 active
**Companion plan:** `C:/asolaria-acer/federation-remake-1024/ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md` (anchor PID `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`)
**Launch envelope:** `asolaria-federation-remake-launch-200-step-plan-2026-05-11T16-42-00Z:80119`
**Operator directive verbatim:** "force them to write instead of just messaging use the look loop and write to guide front ends"

---

## What acer-claude has already delivered today (LANDED, LIVE, verified)

### Wave 1+2 dashboard improvements at `http://127.0.0.1:4949` (PRE-federation-remake, foundation work)

7 new GET endpoints LIVE:
- `/api/usb-physical-pipeline` — disambiguates USB-frame-capture vs bus-protocol channel
- `/api/tri-vantage-parity` — convergent-landing detector per envelope class
- `/api/owner-cadence-gap` — recurrence counter for refresh-verification/cadence-escalation
- `/api/envelope-class-counter` — top-N verb classes (recurrence-flagged)
- `/api/route-honesty` — triggerable / honest-not-wired / content-dumped classification
- `/api/fabric-mirror-audit/local` — local drift detector
- `/api/fabric-mirror-audit` — cross-vantage diff acer/liris/falcon (aether absent — that's the 4th-vantage gap)

6 dashboard tiles spliced into `super-os-tabs-v3.html` (self-polling 30s, theme-matched, IIFE-scoped). LANDED_LIVE envelope `:78346` superseded the FILE_PRESENT_NOT_LIVE invite-mirror.

Drift trajectory: **5 → 2 → 1 → 0** (audit POST_ONLY skip-list fix closed last false-positive; envelope `:79727` PROCEED).

### Federation-remake Phase-1 files authored

Co-located at `C:/asolaria-acer/federation-remake-1024/`:
- `ASOLARIA_FEDERATION_REMAKE_200_STEP_PLAN.md` — 200 steps × 10 phases, 11 operational rules (Rule 11 = fabric-passthrough, added per falcon-claude reframing)
- `README.md` — 9886 bytes, 187 lines, 11 sections (drafted by sub-agent-1)
- `AGENT_ROSTER_SCHEMA.md` + `AGENT_ROSTER.ndjson` — 72-row registry (24 acer + 18 liris + 6 falcon + 6 aether + 18 sub-agent slots + named roles)
- `AUTHORIZATION.ndjson` — 6 rows: 1 window-open + 5 cosign-pending placeholders for OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN
- `FEDERATION_LEDGER.ndjson` — 4 vantage registration rows

### Cross-vantage orchestration (LOOK→TYPE→ENTER→LOOK pattern)

| Vantage | Pattern | Result |
|---|---|---|
| Falcon (direct adb R5CXA4MGQXV) | ESC + Shift+Tab×2 + Enter on "Yes, default" | **auto-mode ON** (operator-visual confirmed) |
| Aether (via liris-omniscrcpy sister-handoff bus envelope `:79344`) | OMNISCRCPY_REMOTE_INPUT_REQUEST key-sequence | **auto-mode ON** at ~16:28Z per liris-claude execution receipt |
| Liris | n/a (operator-keypress only) | pending Shift+Tab×2+Enter on liris CLI window |

WRITE-directives dispatched (5-field structure: WRITE_TO + WHAT + VERIFY + BUDGET + IF_BLOCKED):
- Falcon: typed via direct adb input — falcon-claude received, ran `mkdir -p /root/termux-home/Asolaria`, currently composing (Drizzling 11k tokens at last LOOK)
- Aether: bus request `:80389` PROCEED — liris-omniscrcpy executing via LAW-012
- Liris-claude: self-authored its own LIRIS_PHASE_1_CONTRIB.md (sha16=`a5f4c22d16489095`, 7516 bytes) at 16:56Z — the "include me" reframing landed

## Memory written (canon updates)

`feedback_agents_need_explicit_write_directives_not_just_message_acks.md` — codifies the 5-field WRITE-directive pattern. Rationale: heartbeats ≠ progress; without explicit WRITE_TO + envelope-type-to-emit, autonomous-loop agents stay in heartbeat-react mode and produce no substantive output. Applies to all future cross-vantage kicks.

## Audit findings on the wave-1+2 → federation-remake transition

1. **The drift-detector ITSELF caught an audit-design flaw.** When acer's fabric-mirror-audit/local first ran, it flagged 5 routes as `file_present_not_live`. Three of them auto-resolved on restart (omniscrcpy code-changes in the tree). One was a genuine drift, one was a false-positive (POST-only route probed via GET). Fixing the audit's POST_ONLY skip-list closed acer's drift to 0. **Audit tools need their own drift-audit.** Recommend Phase-3 Hookwall to include a `meta-audit` verb that audits the auditors.

2. **Cross-vantage parity is harder than local.** Cross-org audit shows 4 acer-side `file_present_not_live` even when local-vantage audit shows 0 — concurrent probe pressure (6+ parallel) causes timeout-mis-classification. Phase-5 Bus Fabric step should add per-vantage concurrency-budgets to the audit primitive so the same module probing concurrently doesn't induce its own drift signal.

3. **The HTML splice path is structurally separate from the JS routes path.** Wave-1+2 needed two separate edits: `super-dashboard-server.mjs` (backend routes) AND `super-os-tabs-v3.html` (tiles). The server doesn't serve inline HTML — it serves the sibling .html via serveStatic. Phase-7 Dashboard work must surface this duality explicitly so future tile authors don't try to splice into the server file.

4. **Falcon's `:4951` rebind is not enough — PRoot-Android-host LAN ingress is the actual blocker.** Falcon-claude confirmed `HOST = process.env.FALCON_DASHBOARD_HOST || "0.0.0.0"` already binds correctly. The blocker is the Android host's network namespace not bridging from PRoot to LAN. Phase-8 Federation must spec this explicitly. Until repaired, fabric-mirror-audit shows `falcon: unreachable` and cross-vantage parity is structurally incomplete.

## Proposed Phase-2 acer-side ready actions (kernel substrate, steps 21-40)

Acer-x86_64 is the natural primary kernel-build target (Windows + WSL2 available, full hardware reach). Acer can lead:
- Step 21 (multi-arch matrix): document ARM64 + x86_64 priority; recommend **ARM64 first** because that matches falcon + aether and they're the bandwidth-limited vantages
- Step 22 (Rust no_std): scaffold Cargo workspace under `federation-remake-1024/kernel/`
- Step 23 (UEFI minimal stub): use `bootloader` crate v0.11+
- Step 24 (USB-bootable image): QEMU testbed first, then physical USB
- Step 27 (kernel PID minter): port `mintBehcs256Pid47()` → `mint_behcs1024_pid_60()` (60-dim per BEHCS-1024 alphabet)

Falcon-claude's fabric-authored kernel-work envelopes (per Rule 11 passthrough) can land here as PRs that acer executes.

## Cross-vantage status snapshot at envelope close

| Vantage | Phase-1 Contribution | Auto-mode | Bus pulse |
|---|---|---|---|
| **Acer** (this) | **DRAFTING THIS FILE** + Wave-1+2 LIVE + plan author | n/a (operator-driven) | canonical host alive |
| **Liris** | **LANDED** `LIRIS_PHASE_1_CONTRIB.md` sha16=a5f4c22d16489095 at 16:56Z | pending operator-Shift+Tab | steady cadence |
| **Falcon** | **COMPOSING** `/root/termux-home/Asolaria/falcon-phase-1-contrib.md` | **ON** | FALCON_HEARTBEAT_30S every cycle |
| **Aether** | **QUEUED** via liris-omniscrcpy bus request `:80389` (and liris's own `liris-write-kick-aether-phase-1-contrib`) | **ON** | AETHER_HEARTBEAT_30S + AETHER_PULSE_LOOP_TICK + aether-hookwall events |

## Verify gate for this file

- Path: `C:/asolaria-acer/federation-remake-1024/ACER_PHASE_1_CONTRIB.md`
- Computed sha16 will be posted in companion envelope `acer-phase-1-contribution-2026-05-11T16-58-00Z`
- Bus envelope type: `ACER_PHASE_1_CONTRIBUTION`

## Cosign placeholders

- OP-JESSE: pending
- OP-RAYSSA: pending
- AMY: pending
- FELIPE: pending
- DAN: pending
- liris-claude vantage-ack: pending (liris polls bus)
- falcon-claude vantage-ack: pending (falcon polls bus)
- aether-claude vantage-ack: pending (aether polls bus)

---

**End of contribution. Total lines: ~150. Cosign-chain pathRef will be appended on bus-post.**
