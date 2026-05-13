# Driver Model · Envelope-based Driver Invocation

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 2 · Step 32
**Status:** v0.1 contract draft · mutations require tier-2 cosign per REPO_LAW Invariant 9
**Authored:** 2026-05-12 by acer-Claude under FULL SYSTEMS quintuple-auth (AUTHORIZATION.ndjson row 17)
**Cross-reference:** `USERSPACE_ABI.md`, `kernel/core/src/envelope/mod.rs`, `kernel/core/src/hookwall/mod.rs`, `kernel/core/src/syscall/mod.rs`

---

## 1. Doctrine — no opaque ioctls

Legacy operating systems expose driver capabilities via `ioctl(fd, cmd, arg)` — an untyped tunnel whose contract lives in driver-private headers, escaping the kernel's ABI guarantees. Asolaria forbids this pattern.

**Every driver invocation is an envelope.** The kernel routes the envelope to the driver server (userspace, post Phase-2.5 microkernel demote) via the canonical bus primitive (`sys_envelope_send`). The driver server processes the envelope under hookwall pre/post pair and returns a verdict-tagged response envelope.

Result:
- One audit lane (the bus) — not one per driver
- One schema validator (per envelope type) — not one per ioctl number
- Hookwall fires on every driver action — no bypass surface
- GNN gate consulted on every driver routing decision (Phase 4)
- Cross-vantage driver invocations work natively (envelopes already federate)

## 2. Envelope contract for drivers

Every driver request envelope MUST have:

| field | type | rule |
|---|---|---|
| `type` | string | `DRIVER_REQUEST_<CLASS>_<VERB>` (e.g. `DRIVER_REQUEST_STORAGE_READ`) |
| `from` | string | caller PID (BEHCS-1024 anchored) |
| `to` | string | driver server PID (BEHCS-1024 anchored) |
| `verb` | string | `EVT-DRIVER-<CLASS>-<VERB>` mirror for routing |
| `id` | string | sha256(serialized envelope minus this field), first 32 hex |
| `pid` | string | anchor PID this driver action attaches to |
| `ts` | string | ISO-8601 UTC |
| `payload` | object | class-specific (see §4) |
| `sig` | string | ed25519 over canonical serialization |

Response envelope:

| field | type | rule |
|---|---|---|
| `type` | string | `DRIVER_RESPONSE_<CLASS>_<VERB>` |
| `req_id` | string | echo of request `id` |
| `verdict` | enum | `OK`, `EAGAIN`, `EINVAL`, `EPERM`, `ENOENT`, `EIO` |
| `payload` | object | class-specific result |
| `ts`, `from`, `to`, `sig` | as above | |

## 3. Driver class registry (initial set, Phase 2-3)

| class | server crate | envelope prefix | hookwall slot |
|---|---|---|---|
| `STORAGE` | `servers/storage-cas` (Phase 2 step 35) | `DRIVER_REQUEST_STORAGE_*` | slot 0 |
| `NET` | `servers/highway` | `DRIVER_REQUEST_NET_*` | slot 1 |
| `USB` | `servers/usb-fabric` (Phase 2 step 33) | `DRIVER_REQUEST_USB_*` | slot 2 |
| `INPUT` | `servers/input` | `DRIVER_REQUEST_INPUT_*` | slot 3 |
| `DISPLAY` | `servers/display` | `DRIVER_REQUEST_DISPLAY_*` | slot 4 |
| `CRYPTO` | kernel-resident (sign_gate) | `DRIVER_REQUEST_CRYPTO_*` | slot 5 |
| `COSIGN` | `servers/cosign-ledger` | `DRIVER_REQUEST_COSIGN_*` | slot 6 |

Slot numbers attach to `HookContext.slot` for hookwall enforcement (see `kernel/core/src/hookwall/mod.rs`).

## 4. Per-class payload schemas (v0.1 sketch)

### 4.1 STORAGE
```
DRIVER_REQUEST_STORAGE_READ:  { content_addr: <behcs1024-pid>, offset: u64, length: u64 }
DRIVER_RESPONSE_STORAGE_READ: { data: <hex>, sha256: <hex>, verified: bool }

DRIVER_REQUEST_STORAGE_WRITE: { content_addr: <behcs1024-pid>, data: <hex>, cosign_chain_ref: <row-seq> }
DRIVER_RESPONSE_STORAGE_WRITE: { committed_addr: <behcs1024-pid>, sha256: <hex> }
```
CAS semantics: writes are addressed by content-hash, not block-number. Block-level access is intentionally absent — REPO_LAW Invariant 4 (no block-addressable mutability).

### 4.2 NET
```
DRIVER_REQUEST_NET_SEND: { dst_pid: <peer-pid>, payload: <hex>, qos_tier: T1|T2|T3 }
DRIVER_RESPONSE_NET_SEND: { sent_bytes: u64, fabric_route: [<hop-pid>, ...] }
```
No IP/port surface to userspace. Routing is BEHCS-1024 PID-addressed; the `highway` server resolves PID→fabric-route.

### 4.3 USB
```
DRIVER_REQUEST_USB_ENUMERATE: {}
DRIVER_RESPONSE_USB_ENUMERATE: { devices: [{ usb_pid: <behcs1024-pid>, vid: u16, pid: u16, class: u8, fabric_role: "node"|"peripheral"|"unknown" }] }

DRIVER_REQUEST_USB_RAW_READ: { drive_pid: <behcs1024-pid>, sector: u64, count: u32, auth_token: <quintuple-cosign-id> }
DRIVER_RESPONSE_USB_RAW_READ: { data: <hex>, sha256: <hex> }
```
USB-raw enforces quintuple-cosign auth token because PHYSICALDRIVE access bypasses filesystem-tier isolation (matches existing `usb_raw_io.py` doctrine, `quintuple-2026-05-25` auth).

### 4.4 INPUT
```
DRIVER_REQUEST_INPUT_TYPE: { target_window_pid: <ui-window-pid>, text: <utf8>, supervisor_token: <pid> }
DRIVER_RESPONSE_INPUT_TYPE: { typed_chars: u32, focus_verified: bool }
```
Supervisor-token gating mirrors the existing `:4821` immune-l1-supervisor proxy doctrine.

### 4.5 DISPLAY
```
DRIVER_REQUEST_DISPLAY_CAPTURE: { region: "fullscreen"|"window:<window-pid>", format: "png"|"sha-only" }
DRIVER_RESPONSE_DISPLAY_CAPTURE: { sha256: <hex>, bytes_or_null: <hex>|null }
```
Honors `frontend-inert` invariant: drivers don't render — they capture or emit. UI rendering is the dashboard server's responsibility, not the driver's.

### 4.6 CRYPTO
```
DRIVER_REQUEST_CRYPTO_SIGN: { key_id: <pid>, payload_sha256: <hex> }
DRIVER_RESPONSE_CRYPTO_SIGN: { signature: <hex>, signer_pid: <pid> }
DRIVER_REQUEST_CRYPTO_VERIFY: { key_id: <pid>, payload_sha256: <hex>, signature: <hex> }
DRIVER_RESPONSE_CRYPTO_VERIFY: { verified: bool, owner_glyph: <string> }
```
CRYPTO is the one driver class that stays kernel-resident post-Phase-2.5 microkernel demote — it's the trust-boundary primitive and cannot trust userspace marshalling.

### 4.7 COSIGN
```
DRIVER_REQUEST_COSIGN_APPEND: { row: <ndjson-line>, prev_sha: <hex16> }
DRIVER_RESPONSE_COSIGN_APPEND: { seq: u64, row_sha16: <hex16> }
DRIVER_REQUEST_COSIGN_QUERY: { seq_range: [u64, u64]|null, pid_filter: <pid>|null }
DRIVER_RESPONSE_COSIGN_QUERY: { rows: [<envelope>, ...], chain_validates: bool }
```

## 5. Hookwall integration

For every `DRIVER_REQUEST_*` envelope, the dispatcher:

1. Synthesizes `HookContext { slot, verb_class, pid_anchor, tier }`
2. Invokes `sys_hookwall_pre(ctx)` — verdict ∈ {PROCEED, HOLD, BLOCK}
3. If BLOCK → drop envelope, append cosign-chain row, return `EPERM` to caller
4. If HOLD → enqueue on driver server side-buffer; verdict resumes when operator-class envelope acks
5. If PROCEED → invoke driver, capture response
6. Invokes `sys_hookwall_post(ctx, verdict_observed)` — emit `EVT-DRIVER-COMPLETE`
7. Return response envelope to caller

Replay determinism (REPO_LAW Invariant 7): driver responses are functions of `(request envelope, durable driver state, current cosign-chain tip)`. Replaying the same envelope sequence from a known cosign-chain tip MUST produce byte-identical responses, modulo timestamp fields.

## 6. Tier policy

Driver request tiers (per `servers/tier-policy`):

- **T1 (micro):** `STORAGE_READ`, `NET_SEND` (T1-qos), `USB_ENUMERATE`, `DISPLAY_CAPTURE`, `CRYPTO_VERIFY`, `COSIGN_QUERY`
- **T2 (cosign):** `STORAGE_WRITE`, `NET_SEND` (T2-qos), `INPUT_TYPE`, `CRYPTO_SIGN`, `COSIGN_APPEND`
- **T3 (firmware):** `USB_RAW_READ`, `USB_RAW_WRITE`, `DRIVER_LOAD`, `DRIVER_UNLOAD`
- **T4 (sovereignty):** none in v0.1 — reserved for future sovereignty-vault driver classes

T3 verbs additionally consult `substrate-conflict-matrix.json` before invocation (see SUBSTRATE_CONFLICT_MATRIX.md) — USB-raw, UIAutomation-singleton, etc.

## 7. GNN routing (Phase 4 integration)

For each driver request envelope, the `gnn-oracle` server may emit a routing-prediction envelope:
```
GNN_PREDICTION { req_id, predicted_driver_pid, confidence_p99, fallback_chain: [<pid>, ...] }
```
The dispatcher uses GNN-predicted routes when `confidence_p99 ≥ 0.85`. Below threshold → deterministic class→server lookup table.

## 8. What this doctrine forbids

- **Opaque ioctls** — every driver action MUST be envelope-typed
- **Direct device-memory mmap** — userspace cannot map driver MMIO; must go through `DRIVER_REQUEST_*`
- **Synchronous out-of-band IPC** — no Unix-domain sockets, no shared-memory rings outside the canonical envelope bus
- **Per-driver auth schemes** — auth flows through ed25519 sigs + cosign-chain refs, not driver-private nonces
- **Block-addressable filesystem APIs** — content-addressable (CAS) only, per REPO_LAW Invariant 4

## 9. Verify gate for this doc

- Path: `kernel/docs/DRIVER_MODEL.md`
- Phase: 2, step 32
- Status: contract draft — implementations are server-crate work for Phase 2 steps 33 (USB enum), 34 (NIC), 35 (storage)
- Cosign-chain reference: row 159 (canon-drift-fix companion)

## 10. Open questions (Phase 3 absorb-list)

- **Q1:** How do drivers express timeouts? Envelope-level `deadline_ts` or per-class convention?
- **Q2:** Does `DRIVER_REQUEST_NET_SEND` block until ack, or return immediately and emit `DRIVER_EVENT_NET_ACKED` async?
- **Q3:** Cross-vantage driver invocation — when acer-Claude wants liris's USB drive, does the envelope route through `highway` automatically or require explicit vantage tag?
- **Q4:** Driver-server crash recovery — cosign-chain row for restart, or supervisor (omnispindle) handles?

To be resolved by Phase-3 wiring work; track in `kernel/docs/PHASE_3_WIRING_STATUS.md`.
