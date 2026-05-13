# AGENT_ROSTER.ndjson — Schema

Anchored at PID `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`.

One JSON object per line. UTF-8, LF newline terminators, no trailing comma, no comments.
Consumers MUST treat unknown fields as forward-compatible additions and ignore them.

## Row shape

```json
{
  "id": "agent-<vantage>-<role>-<index>",
  "pid": "<BEHCS-1024 anchor>",
  "vantage": "acer | liris | falcon | aether | this-session",
  "role": "Hermes | Space-deck-driver | Space-agent | PI | Omnispindle | Omniflywheel | Big-Pickle | sub-agent | operator-witness | resident",
  "tier": "T1 | T2 | T3",
  "status": "registered | active | retired",
  "registered_at": "<iso8601 Z>",
  "registered_by": "<principal id>",
  "phase_assignments": [1,2,3],
  "verify_envelope_pathRef": "<bus pathRef when registration cosign-confirmed>"
}
```

## Field reference

| Field | Type | Notes |
|---|---|---|
| `id` | string | Stable identifier. Pattern `agent-<vantage>-<role-slug>-<index>`. Role-slug lowercase, dashes only. Index zero-padded to 2 digits. |
| `pid` | string | BEHCS-1024 anchor. Pattern `AGENT-PID-H<6-hex>-A<vantage-code>-N<index>`. Vantage codes: `01`=acer, `02`=liris, `03`=falcon, `04`=aether, `05`=this-session. |
| `vantage` | enum | Where the agent runs. `this-session` denotes ephemeral sub-agents spawned by an acer-claude session. |
| `role` | enum | Named role or generic class. Named roles (`Hermes`, `Space-deck-driver`, `Space-agent`, `PI`, `Omnispindle`, `Omniflywheel`, `Big-Pickle`) are singletons unless multi-instance is explicitly cosigned. `sub-agent` is the ephemeral task-runner class. `operator-witness` is reserved for Big-Pickle. `resident` is the catch-all for vantage-resident agents without a named role. |
| `tier` | enum | T1 = federation singletons (Hermes, Omnispindle, Omniflywheel, Big-Pickle, PI). T2 = vantage leads (Space-deck-driver, Space-agent, vantage residents). T3 = sub-agents / ephemerals. |
| `status` | enum | `registered` = entry exists, no live process. `active` = process attached, heartbeats ack'd. `retired` = ended (kept for ledger). |
| `registered_at` | string | ISO-8601 UTC. Initial seed uses `2026-05-11T16:45:00Z`. |
| `registered_by` | string | Principal that wrote the row. Initial seed = `ACER-CODEX-FRONTEND-VISUAL-OPERATOR`. |
| `phase_assignments` | int[] | Phases (1..10) the agent is expected to participate in. Empty array = on-call only. |
| `verify_envelope_pathRef` | string\|null | Filesystem or bus pathRef of the cosign-confirmation envelope. `null` until the row is cosigned. |

## Status transitions

- `registered` → `active`: agent emits a `HELLO` envelope on the bus, cosigner countersigns, `verify_envelope_pathRef` is set, status flips. Triggered by Omnispindle.
- `active` → `retired`: agent emits `GOODBYE` envelope, or 3 missed heartbeat windows, or operator-witness retires it. Row is kept; do NOT delete.
- `registered` → `retired`: allowed if the slot is never claimed and superseded by a later cosign.

## Singletons and reservations

- `Hermes` MUST be unique federation-wide. Coordinator role; vantage=acer.
- `Omnispindle` (supervisor) and `Omniflywheel` (prof / LLM router) MUST be unique. vantage=acer.
- `Big-Pickle` is operator-witness only — registered but reserved; status MUST stay `registered` until explicit operator cosign promotes it.
- `PI`, `Space-deck-driver`, `Space-agent` are seeded as singletons on acer; vantage migration requires 5-cosign per Rule 1 of the plan.

## Counts (initial seed)

| Vantage | Count | Notes |
|---|---|---|
| acer | 24 | Hermes, Space-deck-driver, Space-agent, PI, Omnispindle, Omniflywheel, Big-Pickle + 17 residents |
| liris | 18 | Vantage residents |
| falcon | 6 | Vantage residents |
| aether | 6 | Vantage residents (Felipe's Galaxy A06) |
| this-session | 18 | Ephemeral sub-agents |
| **total** | **72** | |

## Example row (literal)

```
{"id":"agent-acer-hermes-01","pid":"AGENT-PID-H7A1B02-A01-N01","vantage":"acer","role":"Hermes","tier":"T1","status":"registered","registered_at":"2026-05-11T16:45:00Z","registered_by":"ACER-CODEX-FRONTEND-VISUAL-OPERATOR","phase_assignments":[1,2,4,5,10],"verify_envelope_pathRef":null}
```

## Append rules

- Never rewrite existing rows; append-only.
- Mutations (status flip, pathRef set) MUST be expressed as a new row with same `id`+`pid` and a fresh `registered_at`; readers fold by taking the last row per `id`.
- File hash MUST be cosigned into COSIGN_CHAIN.ndjson when seed lands.
