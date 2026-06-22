# Hilbra Host8 Integration Receipt

Date: 2026-06-22
Branch: `liris/host8-hilbra-integration-2026-06-22`
Base: `origin/acer/link-auth-gate` (`4c75bb2`)

## Purpose

This branch is the Host8 / Hilbra integration spine for the next Rust-first build phase.
It consolidates the already-stacked Acer Rust work into one reviewable branch before any
launch-side wiring, inverted-index work, or 100B replay.

This is a dry integration receipt only:

- no `fire=1`
- no process launch
- no 100B run
- no substrate write
- no USB / SOVLINUX mutation
- `auto_fire_allowed=false`

## Included Spine

The integration base already contains the following linear Rust spine:

| commit | branch surface | role |
|---|---|---|
| `aae9360` | `acer/commit-host8-serve-rust-2026-06-20` | Host8 serve crate baseline |
| `a41dd45` | `acer/host8-gnn-pixels-first-port` | pixels-first GNN frame wiring |
| `6cd9856` | `acer/agent-runtime-spawn-class-counters` | real registry plus separate dispatch counters |
| `e7f6650` | `acer/fedenv-envelope-validator` | FEDENV-v1 validation and target resolution |
| `cec9498` | `acer/gnn-whiteroom-scorer-omniflywheel` | white-room scorer and Omniflywheel verdict aggregation |
| `6f857ef` | `acer/host8-v1-envelope-wiring` | `/v1/envelope.hbp` dry routing and per-layer counters |
| `83f5056` | `acer/cd-substrate-rooms` | C/D substrate room routing planner |
| `c861d41` | `acer/spawn-gate-ring` | sign gate + hookwall + reverse-gain spawn verdict |
| `0db998e` | `acer/runner-lane-table` | OpenCode / Hermes runner lane table |
| `498ae37` | `acer/fleet-capacity-20k` | 10k-per-substrate / 20k total capacity contract |
| `0fa96a7` | `acer/link-auth-gate` | key-off-wire HMAC and owner-PID consent |
| `13d8165` | `acer/link-auth-gate` | access-level grants |
| `bd3e222` | `acer/link-auth-gate` | level tagger: public tier PII-free |
| `4c75bb2` | `acer/link-auth-gate` | expanded 45-fragment policy and scrubbed test fixture |

## Measured Gate Input

OPERATOR_OBSERVED from Acer G0:

- `rustc 1.95.0`
- `cargo 1.95.0`
- MSVC via `vcvars` present
- current `link-auth-gate` tree: `cargo test --workspace` => 335 passed, 0 failed, 1 ignored, exit 0
- older `fleet-capacity-20k` tree: `cargo check --workspace` passed; lib tests passed; one doctest blocker was a stale bare codefence already fixed in this integration base

LIRIS_MEASURED:

- branch topology confirms `origin/acer/link-auth-gate` stacks on the Host8 / fleet-capacity spine above.
- key modules are present in repo bytes: `link_auth`, `level_tag`, `spawn_gate`, `envelope`, `cosign_chain`, `rooms`, `runners`, Host8 `/v1/envelope.hbp`, and `/summon.hbp`.

## Next Gates

1. Run Acer-side full `cargo check --workspace` and `cargo test --workspace` on this integration branch.
2. If green, treat this branch as #21 complete.
3. Proceed to #22: BEHCS-native, tier-aware inverted index replacing the current synchronous O(N) Recall scan.
4. Keep #19 launch-side wiring behind explicit `fire=1`, spawn gate, cosign seal, and operator T0.
5. Keep #18 controlled live 100B run blocked by #26 replay-prep and operator T0.

## #21 Verdict

`MEASURED_READY_FOR_ACER_RETEST`

The branch is byte-consolidated and dry. Acer must run the final workspace test count because Liris lacks the full Acer MSVC test environment.
