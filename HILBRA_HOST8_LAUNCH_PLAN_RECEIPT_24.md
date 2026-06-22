# Host8 launch-plan receipt — task #24 (acer, overnight 2026-06-22)

Branch: `acer/host8-launch-plan-24`
Base:   `acer/link-auth-gate` (`4c75bb2`, G0 = MEASURED_GREEN 335/0)

## What changed (additive, DRY, ZERO fire)

`servers/host8-serve/src/main.rs` — two NEW additive routes that compose the three E=0 contracts
(`rooms` + `runners` + kernel `spawn_gate`) the build queue had built but never connected:

- **`/launch-plan.hbp?h=<handle8>&device=&ts=&role=<hermes|sub>&score=<q>&risk=<q>`**
  one summon → C/D room (`rooms::room_id_from_pid` → rotating C: room, rename-before-load = $0)
  → runner lane (`runners::runner_for_role`: OpenCode $0 lane / Hermes)
  → spawn-gate ring verdict (`spawn_gate::spawn_gate_verdict`, BLOCK > HOLD > PROCEED)
  → 16-byte HVD **sealed** HBP receipt (`json=0`).

- **`/summon-batch.hbp?count=<N>&role=&score=&risk=`**
  the same plan for the FIRST N seats (cap 1000/req), with per-verdict + per-substrate tallies.
  The operator's "10k and 10k prisms flowing on C and D" expressed as a batch **plan**, not a launcher.

## Hard boundary (the whole point)

- `process_launch=0` **ALWAYS** on both routes — they NEVER spawn a process, rename a folder, or write a substrate.
- `fire_allowed=1` **iff** the gate PROCEEDs (forward score ≥ genius `720` AND reverse risk ≤ `280`).
- Default forward score = the latest **GNN frame score** (pixels-before-GPU); overridable via `&score=` for parity/testing.
- Default (no score) verdict = **HOLD** → `fire_allowed=0`.
- `render_summon`'s existing `&fire=1` path is **UNCHANGED**. Wiring the gate INTO the actual fire (so `fire=1`
  only fires when the gate PROCEEDs) is the launch wave (**#19**) — gated, operator-T0, NOT in this branch.

## Measured (acer, MSVC, rustc/cargo 1.95.0)

`cargo test -p asolaria-host8-serve` → **20 passed / 0 failed / 0 ignored / 0 warnings / exit 0**.

New tests: `launch_plan_composes_room_runner_gate_without_firing`,
`launch_plan_gate_proceeds_on_genius_score_but_still_does_not_launch`,
`launch_plan_hermes_role_selects_hermes_lane`, `launch_plan_unknown_handle_is_404`,
`summon_batch_plans_n_dry_with_tallies`.

## Verdict

`MEASURED_GREEN_ACER` · DRY · E=0 · `auto_fire_allowed=false`. Ready for liris morning attack-verify.

## Next (morning)

1. liris attack-verify this branch (byte-review; full `cargo test` is acer-side — liris lacks MSVC `link.exe`).
2. **#19** — wire the gate into `render_summon`'s fire path (gated, operator T0).
3. **#25** shadow-parity can use these dry plans as the deterministic replay surface.
