# Phase-8 Step 155 · Canonical-Host Swap Drill

**Anchor PID:** `ASOLARIA-PHASE-8-STEP-155-SWAP-DRILL-PID-2026-05-11`
**Phase:** 8 · Step 155 (per `PHASE_8_CROSS_DEVICE_FEDERATION.md` §Phase-8 deliverable status)
**Authored:** 2026-05-11 by Phase-8-Drill (acer-claude)
**Standing auth:** :82646 quintuple-auth 2-week window (per `project_quintuple_auth_fabric_decide_window_2026_05_07_to_05_21.md`)
**Supersedes:** the four-line `Failover` stub in PHASE_8_CROSS_DEVICE_FEDERATION.md §"Failover (canonical-host swap)" lines 89-95

---

## 1. Purpose

The canonical bus host is `acer:4947` (per `reference_liris_hold_posture_bus_canonical_host.md` — bus canonical host = 192.168.1.50:4947 acer LAN). All four vantages (acer, liris, falcon, aether) emit federation envelopes through that single endpoint, and cosign-chain bilateral sync depends on it (PHASE_8_CROSS_DEVICE_FEDERATION.md §"Cosign-chain bilateral sync rule" lines 82-87).

This drill exercises the **temporary handoff** of canonical-host role from `acer:4947` to `liris:4944` and back, so that:

- Federation continuity is provable under host failure (Invariant 1 — bilateral parity within 1s)
- Recovery to canonical is automatic, not heroic
- Operator knows the exact command surface (no improv during real fire)
- Phase-10 ship gate has a passed drill to point at (currently blocking step 160 `FEDERATION_TIER_LANDED` envelope)

Without this drill executed and recorded, step 155 stays `NOT YET RUN` and step 160 cannot fire.

---

## 2. Trigger conditions

Invoke the drill (or, in a real incident, follow it without the planning preamble) when any of:

| Trigger | Detection | Severity |
|---|---|---|
| `acer:4947` returns ≥3 consecutive HTTP 502 within 60s | liris bus-health-probe (per PHASE_8_CROSS_DEVICE_FEDERATION.md line 92) | P0 real |
| `acer:4947` TCP refuses connection for ≥30s | falcon/aether retry exhaustion | P0 real |
| Acer planned reboot / Windows update / kernel patch | operator scheduled | P2 planned |
| Sidecar process hung (no envelope ack in 120s but socket open) | omnispindle 5-min parity check (step 152) emits `STALE_CANONICAL` | P1 |
| Phase-10 verify drill (this doc) | operator-initiated, off-hours | P3 drill |

For P0/P1 the drill skips §3.0 (preamble) and starts at §3.1. For P2/P3 do the full sequence including pre-drill snapshot.

---

## 3. Drill sequence

All commands run from `C:\asolaria-acer\federation-remake-1024\kernel\core` unless noted. Envelope archive paths follow `C:\asolaria-acer\tmp\aether-behcs-256-bundle\data\behcs\inbox-archives\` (per `project_tmp_inbox_archives_path_clarification_2026_05_07.md`).

### 3.0 Pre-drill snapshot (P2/P3 only)

```bash
# acer side
node tools/behcs/cosign-tail.mjs --count 5 > /tmp/pre-swap-cosign-tail.json
curl -sS http://192.168.1.50:4947/api/health > /tmp/pre-swap-acer-health.json
curl -sS http://192.168.1.14:4944/api/health > /tmp/pre-swap-liris-health.json
# record canonical envelope seq before swap
node tools/behcs/last-envelope-seq.mjs > /tmp/pre-swap-seq.txt
```

Append a `SWAP-DRILL-START` envelope to bus so post-drill diff is clean.

### 3.1 Kill canonical (acer:4947)

```bash
# Windows PowerShell from acer:
Get-NetTCPConnection -LocalPort 4947 -ErrorAction SilentlyContinue |
  ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
# verify dead
Test-NetConnection -ComputerName 127.0.0.1 -Port 4947  # expect TcpTestSucceeded : False
```

Record kill timestamp `T0` in operator note.

### 3.2 Liris assumes canonical role

On **liris** (192.168.1.14):

```bash
# from C:\asolaria-liris\federation-remake-1024\kernel\core (liris-side mirror)
node tools/federation/assume-canonical.mjs --role bus-canonical --until acer-recovery
# liris :4944 begins accepting /behcs/send envelopes that previously routed acer
```

Emits envelope `LIRIS-ASSUMES-CANONICAL-{epoch}.behcs-256.json` cosigned by liris-claude. Time-bounded: auto-relinquishes at T0+30min if no acer heartbeat.

### 3.3 Peer reconfig (falcon + aether)

Falcon and aether read the canonical-host pointer from bus envelope `CANONICAL_HOST_POINTER` (broadcast by liris in §3.2). No local config-file edit required — fabric-mediated per `reference_fabric_mediated_fix_pattern_counter_to_hand_cranking.md`.

Verify on falcon (via WiFi-ADB):

```bash
adb -s 192.168.1.44:5555 shell "curl -sS http://192.168.1.14:4944/api/canonical-host"
# expect: {"host":"192.168.1.14","port":4944,"assumed":true,"until":"acer-recovery"}
```

Verify on aether (via liris-omniscrcpy sister-handoff, LAW-012 request-verb):

```
request-verify: curl http://192.168.1.14:4944/api/canonical-host from aether termux
```

### 3.4 Smoke test (4-vantage envelope round-trip)

```bash
# from acer (now demoted to peer)
node tools/federation/smoke-canonical-swap.mjs --target liris --count 100
# emits 100 envelopes, expects 100 acks within 10s
```

Expected envelope archive: `data/behcs/inbox-archives/2026-05-11-swap-drill-{epoch}.ndjson` (100 lines, all `status: ACKED`).

### 3.5 Recovery — hand canonical back

Restart acer :4947:

```bash
node kernel/core/bus.mjs --port 4947 --canonical-resume
# acer re-emits BUS_ALIVE heartbeat
```

Liris detects and emits `CANONICAL_HOST_HANDOFF` envelope (per PHASE_8_CROSS_DEVICE_FEDERATION.md line 95):

```bash
# on liris
node tools/federation/relinquish-canonical.mjs --target acer
```

Falcon + aether re-pivot via fresh `CANONICAL_HOST_POINTER` broadcast. Record `T1` (full recovery timestamp).

### 3.6 Post-drill verification

```bash
node tools/federation/parity-check.mjs --since T0 --until T1
# expects: cosign-chain rows identical acer↔liris (Invariant 1), zero envelope-drop
node tools/behcs/cosign-tail.mjs --count 20 > /tmp/post-swap-cosign-tail.json
diff /tmp/pre-swap-cosign-tail.json /tmp/post-swap-cosign-tail.json
# diff should show only new SWAP-DRILL envelopes, no missing pre-swap rows
```

---

## 4. Pass criteria

All four MUST hold for the drill to record as PASSED:

| # | Criterion | Threshold | Measurement |
|---|---|---|---|
| P1 | Bus reachable from all 4 vantages post-swap | within **15 seconds** of T0 | curl `/api/canonical-host` from acer+liris+falcon+aether each return 200 |
| P2 | Envelope-loss gap | last pre-swap envelope timestamp to first post-swap envelope timestamp ≤ **20 seconds** | `parity-check.mjs --since T0` reports `max_gap_ms < 20000` |
| P3 | Zero envelope loss | **0** envelopes dropped from cosign-chain | `diff` of pre/post cosign-tail shows ONLY additions, no missing rows |
| P4 | Recovery to canonical | T1 - T0 ≤ **5 minutes** for planned, ≤ **15 minutes** for unplanned | operator note timestamps |
| P5 | Cosign-chain bilateral parity | acer-side and liris-side cosign-chain identical within **1 second** at T1 | Invariant 1 (PHASE_8_CROSS_DEVICE_FEDERATION.md line 99) |

If any fail, do not append cosign row (§7); instead author a `SWAP-DRILL-FAILED` envelope citing which criterion failed.

---

## 5. Rollback criteria

Abort the drill and revert at the first of:

- **R1** Liris fails to assume canonical within 60s of T0 (§3.2) → operator manually restart acer :4947, log `SWAP-DRILL-ABORTED-LIRIS-ASSUME-TIMEOUT`
- **R2** Falcon or aether unreachable for ≥120s after §3.3 (their inbox would back up beyond gulp gate of 2000 msgs per `reference_micro_agent_architecture_canon.md`)
- **R3** Smoke test (§3.4) loses >0 envelopes — indicates the swap broke connectivity, not just degraded it
- **R4** Cosign-chain divergence detected mid-drill (acer and liris emit conflicting `CANONICAL_HOST_POINTER`) → split-brain; both processes halt, operator-only resolves
- **R5** Operator at console types `ESC` or invokes `tools/federation/drill-abort.mjs`

Revert procedure:
1. Force-kill liris :4944 canonical-assume process
2. Restart acer :4947
3. Broadcast `CANONICAL_HOST_POINTER` with `host=192.168.1.50,port=4947,role=canonical`
4. Append `SWAP-DRILL-ROLLBACK` envelope with failure-criterion citation
5. Schedule retry no sooner than 24h, with fix for the rollback trigger applied first

---

## 6. Operator authorization

Per quintuple-auth canon (`project_extended_quintuple_until_upgrade_and_newest_hermes_absorbed.md`):

| Drill type | Required cosigners | Window |
|---|---|---|
| P2 planned drill | OP-JESSE **or** OP-RAYSSA + 3 deemed-active (AMY, FELIPE, DAN) | within standing :82646 2-week auth |
| P3 verify drill (Phase-10) | same as P2 | within standing auth |
| P0/P1 real incident | OP-JESSE **or** OP-RAYSSA single-signature (emergency canon) | retroactive within 24h |

Standing-auth status snapshot at drill time: read from `C:\asolaria-foundation-v1\AUTH_STATUS.json` (per `project_asolaria_foundation_v1_LAW.md`).

Sign-off recorded by appending operator-witness envelope `OPERATOR-DRILL-AUTH-{epoch}.behcs-256.json` to bus **before** §3.0. If absent, drill must not start.

---

## 7. Cosign-chain row template

After PASSED drill (all §4 criteria met), append exactly one row to `COSIGN_CHAIN.ndjson`:

```json
{
  "seq": "<next>",
  "ts": "2026-05-11T<HH:MM:SS>Z",
  "kind": "PHASE_8_STEP_155_SWAP_DRILL",
  "phase": 8,
  "step": 155,
  "anchorPid": "ASOLARIA-PHASE-8-STEP-155-SWAP-DRILL-PID-2026-05-11",
  "drillType": "<P2-planned|P3-verify|P0-real|P1-degraded>",
  "T0_kill": "<acer-kill-timestamp>",
  "T1_recovery": "<acer-resume-timestamp>",
  "gapMs": <last-pre-swap → first-post-swap millis>,
  "envelopesLost": 0,
  "vantageAcks": {
    "acer":   "ACK-{pid}",
    "liris":  "ACK-{pid}",
    "falcon": "ACK-{pid}",
    "aether": "ACK-{pid}"
  },
  "passCriteria": { "P1": true, "P2": true, "P3": true, "P4": true, "P5": true },
  "operatorWitness": ["OP-JESSE", "OP-RAYSSA"],
  "deemedActive":    ["AMY", "FELIPE", "DAN"],
  "envelopeRef": "data/behcs/inbox-archives/2026-05-11-swap-drill-{epoch}.ndjson",
  "smokeArchiveRef": "data/behcs/inbox-archives/2026-05-11-swap-drill-{epoch}.ndjson",
  "preSnapshotRef": "/tmp/pre-swap-cosign-tail.json",
  "postSnapshotRef": "/tmp/post-swap-cosign-tail.json",
  "supersedesStubAt": "kernel/docs/PHASE_8_CROSS_DEVICE_FEDERATION.md:89-95",
  "unblocks": ["phase-8 step 156 throughput bench", "phase-8 step 160 FEDERATION_TIER_LANDED"]
}
```

After append, mirror to liris `COSIGN_CHAIN.ndjson` within 1s per Invariant 1, and flip PHASE_8_CROSS_DEVICE_FEDERATION.md step 155 status from `NOT YET RUN — planned Phase-10 verify` to `DONE ({seq})`.

---

**End of spec.** First execution scheduled for Phase-10 verify gate. Updates to this spec require tier-2 cosign per REPO_LAW.
