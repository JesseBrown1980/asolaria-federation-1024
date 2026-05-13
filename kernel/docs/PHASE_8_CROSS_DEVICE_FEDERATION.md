# Phase-8 · Cross-Device Federation Topology

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 8 · Steps 141-160
**Authored:** 2026-05-11 by acer-claude
**Quintuple-auth window:** :82646 covers (T1-T6 standing 2-week)

---

## Four-vantage federation map (canonical)

```
                      ACER (192.168.1.50)
                      Windows x86_64
                      bus :4947  dashboard :4949
                      canonical bus host
                            ▲ ▼
                ┌───────────┘ └───────────┐
                │ direct-wire             │ federation bus
                │ 1 Gbps Ethernet         │ (logical, via 192.168.1.x)
                ▼                         ▼
        LIRIS (192.168.1.14)       FALCON (192.168.1.44)
        Windows x86_64             Samsung S24FE ARM64
        dashboard :4944            PRoot alpine claude
        sister-organ                 :4951 (localhost-bound)
                                      WiFi-ADB :5555
        USB ────────────► AETHER (USB-tethered via liris)
                          Samsung Galaxy A06 ARM64
                          com.anthropic.claude in Termux
```

## Per-vantage canonical config

| Vantage | Bus endpoint | Dashboard | Claude runtime | PID anchor |
|---|---|---|---|---|
| acer | `http://192.168.1.50:4947` (canonical) | `http://127.0.0.1:4949` | claude-code on Windows | `ACER-PID-H740C` |
| liris | `http://192.168.1.14:4944/behcs/*` (sister) | `http://192.168.1.14:4944` | claude-code on Windows | `LIRIS-PID-H0EF3-A00-W150` |
| falcon | bus-via-WiFi-ADB | `http://127.0.0.1:4951` (PRoot-local) | claude 2.1.126 proot-distro alpine | `FALCON-PID-H8D3F` |
| aether | bus-via-liris-handoff | scrcpy-to-operator-display | com.anthropic.claude / Termux | `AETHER-CLAUDE-PID-G0049-A00-W001` (post-migration) |

## Lane canon (cross-vantage routing)

### Lane 1 — `acer-liris` (direct-wire 1 Gbps Ethernet)
- **mode:** sister-bilateral (HTTP POST envelopes both directions)
- **bandwidth:** ≥ 800 Mbps sustained
- **target round-trip latency:** < 5 ms
- **fallback:** SMB share `\\DESKTOP-PTSQTIE\liris-asolaria\` (read-only from acer)

### Lane 2 — `acer-falcon` (WiFi-ADB)
- **mode:** local-adb-usb (typing-direct + screencap)
- **endpoint:** `192.168.1.44:5555` (adb wireless)
- **scrcpy:** operator-visible display
- **limitation:** falcon's `:4951` localhost-bound — PRoot-Android-host LAN ingress unresolved (per `project_falcon_claude_proot_alpine_shim_2026_05_07.md`)

### Lane 3 — `liris-aether` (sister-handoff via local USB)
- **mode:** sister-handoff-envelope (acer bus → liris-omniscrcpy → USB adb to aether)
- **aether-USB serial:** R9QY205KAKJ
- **request-verbs:** request-look / request-type / request-enter / request-verify (per LAW-012)
- **fallback:** if liris keyboard daemon dead (:4820 ECONNREFUSED), use bus envelope + operator-manual type

### Lane 4 — `acer-aether` (bus-only — no direct typing)
- **mode:** bus envelope to `@AETHER-CLAUDE`
- **read path:** aether-claude polls inbox in autonomous loop
- **no physical kick** — aether-claude's claude.app reads bus messages via Termux-curl

## SMB share canon (per Vanguard scout #2 finding)

| Share | Host | Mode | Use |
|---|---|---|---|
| `\\DESKTOP-J99VCNH\AcerBridge` | acer | RW | cross-host file drop to liris |
| `\\DESKTOP-PTSQTIE\liris-asolaria` | liris | **read-only from acer** | reading liris source for absorb |
| `\\DESKTOP-PTSQTIE\asolaria-bridge` | liris | permission-denied from acer | future write-bridge |
| `\\DESKTOP-PTSQTIE\liris-memory` | liris | unknown | liris memory store |

## Auto-mode propagation status (per :82646 quintuple-auth)

- acer: operator-driven (this session)
- liris: auto-mode ENABLED via operator manual Shift+Tab + quintuple-auth standing approval
- falcon: auto-mode ENABLED via acer adb-type (Shift+Tab×2 + Enter on "Yes default")
- aether: auto-mode ENABLED via liris-omniscrcpy sister-handoff (LAW-012 keysequence)

## Cosign-chain bilateral sync rule

- acer cosign-chain row → liris within 1 second target
- liris cosign-chain row → acer within 1 second target
- falcon append-only (via bus envelope, no local writer)
- aether append-only (via bus envelope, no local writer)

## Failover (canonical-host swap)

If acer :4947 fails:
1. Liris :4944 detects via bus health probe (≥ 3 consecutive 502s in 60s)
2. Liris assumes canonical-bus-host role temporarily
3. Acer envelopes re-route via liris :4944/behcs/send
4. Recovery: acer restart → liris hands canonical back via `CANONICAL_HOST_HANDOFF` envelope

## Federation invariants

1. **Bilateral parity:** acer ↔ liris cosign-chains identical within 1s
2. **4-vantage acks:** major envelopes require 3+ vantage cosigns (Invariant 8 of REPO_LAW)
3. **Sandbox passthrough:** falcon-PRoot + aether-Termux contributions land via fabric IPC (Rule 11)
4. **Operator-witness for T3+:** standing approval via `:82646` quintuple-auth for 2-week window
5. **No bloat:** if a lane goes unused for 7 days, deprecate (no orphan transport infrastructure)

## Phase-8 deliverable status

| Step | Item | Status |
|---|---|---|
| 141 | acer canonical bus host config | DONE (running) |
| 142 | liris sister-organ config | DONE (`:4944` alive 12500s+) |
| 143 | falcon proot-distro alpine claude config | DONE (manual operator boot) |
| 144 | aether Termux + com.anthropic.claude config | DONE (manual operator install) |
| 145 | sister-handoff lane (liris→aether USB) | DONE (active, request-type works) |
| 146 | direct-wire lane (acer→liris 1Gbps) | DONE (active) |
| 147 | SMB fallback | DONE (read-only confirmed) |
| 148 | WiFi-ADB lane (acer→falcon) | DONE (R5CXA4MGQXV active) |
| 149 | Tailscale fallback | DEFERRED (optional per plan) |
| 150 | Auto-mode propagation | DONE on 4/4 vantages (via :82646) |
| 151 | Cosign-chain bilateral sync | RUNNING |
| 152 | Cross-vantage parity check | RUNNING (5min cadence per omnispindle) |
| 153 | Cross-vantage audit | LIVE via `/api/fabric-mirror-audit` |
| 154 | Canonical posture broadcasting | RUNNING (hermes coordinator pending v0.2) |
| 155 | Canonical-host swap drill | NOT YET RUN — planned Phase-10 verify |
| 156 | Federation throughput bench (10K envelopes) | NOT YET RUN |
| 157 | Federation invariants doc | THIS FILE |
| 158 | Federation membership ledger | DONE (`FEDERATION_LEDGER.ndjson`) |
| 159 | Federation deprecation policy | INLINED IN INVARIANT 5 ABOVE |
| 160 | `FEDERATION_TIER_LANDED` envelope | PENDING — needs steps 155-156 first |

## Cosign placeholders

Per `:82646` quintuple-auth: OP-JESSE + OP-RAYSSA + AMY + FELIPE + DAN deemed-active for 2-week window.
- liris-claude vantage-ack: pending
- falcon-claude vantage-ack: pending
- aether-claude vantage-ack: pending

---

**This doc enables Phase-10 ship gate to verify cross-device interconnect against canonical map. Updates require tier-2 cosign per REPO_LAW.**
