# Host8 parity map — Node source/oracle → Rust Host8 target

Date: 2026-06-28 · acer lane (P4B) · **docs-first, E=0.** No `:5088` redeploy, no CI run, no fire.

This repo is the **kernel rung** of the Asolaria root (the watcher-gated, infinitely-nestable 8-byte
agent — see the `ROOT-PRIMITIVE` doc + `MAP.md`). It is the **Node→Rust Host8 upgrade target**. This map
keeps the four states **separate** so "what runs" is never confused with "what is canon in source."

## The four states (never conflate)
| state | meaning | how to confirm |
|---|---|---|
| **SOURCE** | code present in HEAD (here + the Node oracle repos) | `git ls-tree` / repo tree |
| **BUILT** | compiles + tests pass — **SCOPED** unless the owning 1.81 CI ran | `cargo build/test` (default toolchain ≠ owning CI) |
| **RUNNING** | the live `:5088` binary (kernel 0.2.0-phase3-scaffold) — **LAGS HEAD** | `curl :5088` HOST8HDR |
| **LIVE / FIRED** | materialized runtime — **E=0 here** (`spawn_count=0`, `process_launch=0`, `auto_fire=false`) | live probe |

> "in HEAD" ≠ "built" ≠ "running on :5088" ≠ "fired." A claim must name which state it is in.

## Source → Rust target map
| capability | Node source / oracle | Rust Host8 target (this repo) | state (P1-grounded) |
|---|---|---|---|
| 200ns PID emitter | `Asolaria-the-full-works-…emitter` / `bigpickle-rebuild` revolver | kernel spawn path | SOURCE both sides |
| FEDENV dispatcher | `omni-dispatcher` (`validator.mjs`) | `kernel/core` fedenv + `host8-serve /v1/envelope` | ported · 12/12 node-parity (per 2026-06-22 session-update) |
| C/D rooms (rename-before-load) | `project-room-router.mjs` | `servers/agent-runtime/rooms.rs` | in HEAD+tests · **NOT in running :5088** |
| white-room Scorer / Omniflywheel | `Shannon-and-the-gnns-stage` / `…fnns-trained` | `servers/gnn-oracle` | in HEAD |
| spawn-gate ring | hookwall tier + reverse-gain | `kernel/core/spawn_gate` | in HEAD |
| agent registry | — | `servers/agent-runtime` (`spawn_real_gated`, `AGENT_REGISTRY_MAX=10_000`) | `spawn/retire/heartbeat`=stub · `spawn_child_agent`=wired |
| after-100B cubes | `Asolaria-the-after-100-billion-run…` | cube absorption | SOURCE |

Branch **`acer/fleet-capacity-20k`** stacks the Host8 ports (20k fleet capacity).

## Honest parity status (from P1, watcher-gated)
- **RUNNING `:5088` LAGS HEAD:** `launch-plan` / `room-rotor` / `spawn-gate` / `summon-batch` /
  `shadow-parity` / `replay-prep` + the 6-way `DispatchCounters` exist in HEAD **with tests** but are
  **not in the running binary** (≈5-day uptime).
- **No owning 1.81 CI run** has been done in this pass → all parity/threshold claims are **SCOPED**
  (source + tests present), **NOT pipeline-verified**.
- **E=0 proven live:** `spawn_count=0`, `process_launch=0`, `auto_fire=false` on `:5088`.

## Gated (hard hold — operator T0 only; NOT this docs pass)
- The **owning 1.81 CI** (`cargo fmt --all -- --check` + `clippy --workspace -D warnings` + `check`
  workspace + kernel sub-workspace + no-bloat) — converts SCOPED → pipeline-verified.
- **`:5088` redeploy** to match HEAD (gains `launch-plan`/`room-rotor`).

Until then: read the **tree** for canon-in-source, and `:5088` for what-actually-runs — they differ by design (the system is mid-migration).
