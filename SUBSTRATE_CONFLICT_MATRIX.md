# SUBSTRATE_CONFLICT_MATRIX · Phase-2.5.5 step A3

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 2.5.5 · Step A3 · UIAutomation Substrate Absorption
**Author:** acer-Claude (vantage acer · ACER-PID-H9E2A-A07-W104-P00-N00000)
**Authored:** 2026-05-12T18:43:00Z
**Status:** WRITE stage of A3 (first stage of 6-stage write→review→test→review→implement→look loop)
**Companion data:** `substrate-conflict-matrix.json` (machine-readable sidecar for proxy import)
**Trigger:** Liris's spacedesk-deep-walk surfaced the USB-Android-driver / scrcpy-adb contention on the same physical USB cable. Generalized here.

---

## Rule

Every UIAutomation proxy verb (Phase-2.5.5 A1–A10) MUST consult this matrix before invoking its AutomationId. If the resource is currently CLAIMED by another logical substrate AND the operator has not cosigned a swap, the proxy MUST refuse and emit a `SUBSTRATE_CONFLICT` envelope (`body.room='whiteroom'`, `body.tags=['class:CONTROL','verb:substrate-conflict']`).

This is enforced at the proxy layer (userspace) AND audited at the hookwall layer (kernel, per REPO_LAW Invariant 2) so a misbehaving proxy cannot bypass.

---

## Conflict matrix

| # | Physical resource | Competing logical substrates | Coexistence | Resolution policy |
|---|---|---|---|---|
| 1 | **USB cable (Type-C, falcon → acer)** | (a) `adb-scrcpy-mirror` + `omniscrcpy-action` verbs (type/tap/key/exec/relay), (b) `spacedesk:tree:usb-android` ON (pixel-tether), (c) `spacedesk:tree:usb-ios` (would-be iOS, currently UNAVAILABLE) | **EXCLUSIVE** — only ONE logical substrate at a time can claim Android USB endpoint | Incumbent wins. If `adb devices` shows `state=device` for any serial, refuse `spacedesk:tree:usb-android:on`. Operator cosign required to swap. On swap: emit `USB_SUBSTRATE_HANDOFF` envelope, kill incumbent gracefully, then claim. |
| 2 | **LAN ethernet (acer 192.168.1.50 / liris 192.168.1.14)** | (a) bus `:4947`, (b) dashboard `:4949`, (c) liris bus `:4944`, (d) MWB `:15101`, (e) spacedesk LAN-server `:28252`, (f) RBM rig `:48xx`, (g) supervisor proxies `:4820/:4794`, (h) SSH `:22` (acer once up) / `:8022` (falcon termux) | **COEXIST** — different TCP ports, kernel multiplexes. | No conflict. Port-bind collision detected by OS; proxy verb that requests a bound port emits `PORT_BIND_COLLISION` envelope and refuses. |
| 3 | **TPM 2.0 (kernel trust roots)** | (a) ed25519 kernel-trust-roots per Phase-2 step 28, (b) Windows BitLocker, (c) Windows Hello, (d) virtualization-based-security key sealing | **COEXIST** — TPM supports multiple sealed objects in different NV indices. | No conflict at Tier-1. Tier-2 cosign required for any new NV index allocation. |
| 4 | **Physical screen (display output)** | (a) Windows desktop compositor (native), (b) `scrcpy` mirror windows (multiple, one per device), (c) `spacedesk` server output (when LAN viewers connected), (d) `omniscrcpy:screenshot` GDI capture, (e) screen recorders | **COEXIST** — different render layers / composition surfaces. | No conflict. GDI capture is read-only on top of compositor. scrcpy + spacedesk are separate windows. |
| 5 | **Audio capture (microphone)** | (a) Windows default capture, (b) Spacedesk audio-passthrough (if enabled), (c) Termux audio (falcon, via PRoot — none currently), (d) videoconferencing apps | **EXCLUSIVE PER STREAM, COEXIST AT MIXER** — Windows audio mixer multiplexes capture; exclusive-mode apps lock. | Default: exclusive-mode requests denied at proxy unless cosigned. Tier-2 cosign required to claim exclusive-mode. |
| 6 | **GPU compute / shader queue** | (a) Windows desktop compositor, (b) any CUDA / DirectX compute load, (c) GNN inference (Phase-4 step 64, when ggml moves to GPU), (d) scrcpy h264/h265 encode (currently CPU for liris-mirror) | **TIME-SLICED** — GPU scheduler multiplexes. | No hard conflict. Latency budget contention: GNN inference p99 ≤ 100ms (Invariant 3) must hold; proxy verb invoking heavy GPU compute checks current GPU load and defers if >80% per nvidia-smi/Get-Counter. |
| 7 | **filesystem path: USB-SOVLINUX-2TB raw device (`\\.\PHYSICALDRIVE2`)** | (a) `usb_raw_io.py` (acer canonical, this session's tool find), (b) `omniscrcpy-spacedesk-proxy:tree:usb-android` (when USB-Android driver claims the SOVLINUX stick, not the falcon stick), (c) Windows FS mount (drive letter), (d) liris-side `@asolaria/asolaria-exfat-driver` (when USB on liris), (e) Phase-2 step 24 USB-bootable-image builder | **EXCLUSIVE PER WRITE, COEXIST FOR READ** | Reads always permitted (raw + FS mount coexist for read). Writes via raw require unmount/lock (`FSCTL_LOCK_VOLUME` + `FSCTL_DISMOUNT_VOLUME`). Quintuple cosign for any raw write per usb_raw_io.py auth token. |
| 8 | **AnyDesk / TeamViewer / spacedesk REMOTE input control** | (a) operator's local keyboard/mouse, (b) MWB cross-host input, (c) AnyDesk inbound session, (d) spacedesk client when fully connected | **COEXIST IN INPUT QUEUE** — Windows input subsystem queues events from all sources. | No conflict. Tier-3 STEALTH cosign required to enable inbound remote control from a NEW source (proxy verb to enable a remote connection). |
| 9 | **CPU thread budget** | All proxies + scrcpy + spacedesk + dashboard servers + cron ticks + Claude itself | **TIME-SLICED** — OS scheduler multiplexes. | No hard conflict. Soft budget: total user-cpu < 80% sustained; proxy verb that would burst above checks current load via `Get-Counter '\Processor(_Total)\% Processor Time'` and defers if hot. |
| 10 | **AUTOMATION (UIAutomation singleton in user session)** | (a) liris's spacedesk-proxy walking spacedeskConsole, (b) acer's spacedesk-proxy mirror (A5), (c) future Settings-page proxy (A6), (d) future Explorer proxy (A9), (e) any operator manual interaction with the UI | **COEXIST AT READ, SERIALIZE AT WRITE** — UIAutomation is thread-safe for reads; writes (invoke patterns) serialize per-window. | Two proxies invoking the SAME AutomationId within 100ms = race. Proxy must take a per-process-id lockfile (`%TEMP%\omniscrcpy-uia-<pid>.lock`) for any write op. Held across the verb invocation only. |

---

## Compact JSON sidecar shape (for proxy import)

The companion `substrate-conflict-matrix.json` mirrors rows 1–10 in machine-readable form:

```json
{
  "schema_version": "0.1",
  "anchor_pid": "ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11",
  "rows": [
    {
      "id": "USB_CABLE_FALCON_ACER",
      "physical_resource": "USB Type-C cable falcon→acer",
      "competitors": [
        {"name": "adb-scrcpy-mirror",     "verb_prefix": "omniscrcpy:type|tap|key|exec|relay|scrcpy:*"},
        {"name": "spacedesk-usb-android", "verb_prefix": "spacedesk:tree:usb-android"},
        {"name": "spacedesk-usb-ios",     "verb_prefix": "spacedesk:tree:usb-ios"}
      ],
      "coexistence": "EXCLUSIVE",
      "policy": "incumbent_wins_operator_cosign_to_swap",
      "swap_envelope_type": "USB_SUBSTRATE_HANDOFF",
      "incumbent_probe": "adb devices (state=device) OR netsh trace WinUSB endpoint"
    },
    {
      "id": "LAN_TCP_PORTS",
      "physical_resource": "LAN ethernet",
      "coexistence": "COEXIST_DIFFERENT_PORTS",
      "policy": "os_multiplexes",
      "collision_envelope": "PORT_BIND_COLLISION"
    },
    {
      "id": "USB_SOVLINUX_2TB_RAW",
      "physical_resource": "\\\\.\\PHYSICALDRIVE2 (USB-SOVLINUX-2TB)",
      "competitors": [
        {"name": "usb_raw_io.py",                  "verb_prefix": "usb-raw:read|write"},
        {"name": "asolaria-exfat-driver-liris",    "verb_prefix": "exfat:*"},
        {"name": "windows-fs-mount",               "verb_prefix": "explorer:*"},
        {"name": "build-usb-img-script",           "verb_prefix": "kernel:build-img"}
      ],
      "coexistence": "EXCLUSIVE_PER_WRITE_COEXIST_FOR_READ",
      "policy": "raw_writes_require_lock_dismount_plus_quintuple_cosign",
      "auth_token": "quintuple-2026-05-25",
      "swap_envelope_type": "USB_SOVLINUX_LOCK_HANDOFF"
    },
    {
      "id": "UIAUTOMATION_SINGLETON",
      "physical_resource": "UIAutomation tree in user session",
      "coexistence": "COEXIST_AT_READ_SERIALIZE_AT_WRITE",
      "policy": "per_process_lockfile_held_across_invoke",
      "lockfile_pattern": "%TEMP%\\omniscrcpy-uia-<pid>.lock",
      "lock_timeout_ms": 5000
    }
  ]
}
```

(Rows 3, 4, 5, 6, 8, 9 follow the same shape — omitted from the inline JSON for brevity; full sidecar is the companion `.json` file.)

---

## Why this is load-bearing

Without this matrix, the first time a proxy verb tries to flip `spacedesk:tree:usb-android:on` on a session where adb is actively serving scrcpy, the USB device hand-off race will kill scrcpy mid-stream. The operator sees a black mirror window and has to physically replug. The matrix turns that failure into a refusable envelope + cosign request — graceful instead of destructive.

It also makes Phase-9 (Stealth Tiers) enforcement clean: every proxy verb's tier is declared per-AutomationId in the proxy source; the conflict matrix is the orthogonal constraint (physical resource availability) that combines with tier policy to gate the actual invocation.

---

## Next stages of A3 (the loop)

This file is the WRITE stage. Remaining stages:
1. ~~**write**~~ (this file) ← DONE
2. **review** — read this back, check for missing physical resources, check matrix completeness
3. **test** — synthesize a test invocation that should be refused (e.g. `spacedesk:tree:usb-android:on` while adb is serving) and verify the proxy refuses cleanly
4. **review** — second pass after test, fix any matrix entry that the test surfaced as wrong
5. **implement** — add the conflict-check helper module that proxies import (Phase-2.5.5 step A4)
6. **look** — emit ACER-A3-LANDED envelope, check broadcasts/ for vantage-acks, mark task complete

Stages 2–6 execute on subsequent ticks. This tick handles stage 1 only.

---

## Cosign placeholders

- acer-Claude (this author): GRANTED on file write
- liris-Claude: PENDING (Rule-11 fabric-passthrough review; substrate-conflict-aware spacedesk proxy invocation is liris-side first)
- OP-JESSE / OP-RAYSSA: covered under AUTHORIZATION.ndjson rows 7 + 14 (declared blanket quintuple-cosign for ALL 200 STEPS until completion)
- falcon-Claude: PENDING (SIGNAL-VARIANCE attestation per established pattern)

---

**End of A3 WRITE stage · loop continues on next tick · matrix governs Phase-2.5.5 A4 onwards**

---

## REVIEW notes (tick-002, 2026-05-12T18:55Z) — gaps surfaced by BEAST look

1. **Missing row: Samsung-Kies protocol substrate.** `ss_conn_service` + `ss_conn_service2` (SAMSUNG Mobile Connectivity Service) compete with ADB and MTP/WPD for Samsung device USB-endpoint claim. Today found BOTH services stopped on acer. Add as **row 11** — physical resource = Samsung-device USB endpoint, competitors = `adb`, `samsung-kies (ss_conn_service)`, `samsung-smart-switch`, `mtp-wpd`, coexistence = EXCLUSIVE per protocol claim, policy = `incumbent_wins` + service-state probe required.
2. **Missing row: WPD (Windows Portable Devices) class drives.** Today found E:\ enumerates as WPD class, not standard FS — `Get-ChildItem` returns empty even though `Get-Disk` shows 2TB Online. WPD competes with standard filesystem mount semantics. Add as **row 12** — physical resource = USB stick advertising WPD, competitors = `windows-fs-mount`, `wpd-mtp-shell-namespace`, `usb_raw_io.py` (raw bypasses both), coexistence = MUTUALLY-EXCLUSIVE-AT-ENUMERATION-LAYER, policy = raw-IO is canonical when WPD-FS disagrees.
3. **Row 1 (USB cable) under-specified for Samsung-class.** Need to split into "USB cable" + "device-class protocol claim". A Samsung phone on USB can be in ADB / MTP / Kies / charge-only mode — only one logical protocol claims the endpoint, even though the physical cable could carry any. Refactor row 1 to distinguish cable-layer vs protocol-layer.
4. **JSON sidecar incomplete.** Rows 3, 4, 5, 6, 8, 9 not expanded in the sidecar example. Full sidecar file `substrate-conflict-matrix.json` still needs authoring (separate write op, A3 implement-stage).
5. **No row for sovereign-vantage routing.** When acer talks to BEAST via Falcon hookwall-v2 fan-out (TCP :34176 ESTABLISHED canonical) AND simultaneously via direct LAN POST `:4799/exec`, those are competing logical channels to the same device. Add **row 13** — resource = BEAST-device-control, competitors = direct LAN, hookwall-v2-fan-out, omniscrcpy-broadcasts-relay, coexistence = COEXIST-AT-DELIVERY-FAN-OUT-OK-AT-LISTENER, policy = idempotent envelope IDs prevent double-execution.

REVIEW verdict: matrix is structurally sound but needs 3 new rows (11, 12, 13) + row 1 refactor + JSON sidecar completion. Test stage (tick-003) will exercise the USB-cable conflict using actual `adb devices` + `Get-Service ss_conn_service` probes.

