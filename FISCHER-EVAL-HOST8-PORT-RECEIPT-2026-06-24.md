# Fischer Eval Host-8 Port Receipt — Liris Side

**Date:** 2026-06-24  
**Branch intent:** additive Rust Host-8 migration build, no live cutover.

## Scope

This branch adds `servers/fischer-eval`, a Rust Host-8 port of the live Fischer evaluator kernel.
It is not a replacement of the live Node runtime yet.

- `MEASURED`: live Fischer remains reachable at `127.0.0.1:4794` and reports `FISCHER-LIVE|ok=1|...|json=0`.
- `MEASURED`: Node ground-truth source lives under `C:\Users\rayss\_bigpickle_acer_fischer\src\`.
- `MEASURED`: Node ground-truth unit suite passes `26/26`.
- `MEASURED`: Rust `cargo +1.81 check -p asolaria-server-fischer-eval --tests` passes.
- `MEASURED`: Rust `cargo +1.81 clippy -p asolaria-server-fischer-eval --tests -- -D warnings` passes after fixing the two Acer-reported lints.
- `MEASURED`: static scan of the Rust source found no JSON hot-path markers.
- `MEASURED`: exact owning fmt gate `cargo +1.81 fmt --all -- --check` passes after applying 1.81 rustfmt to the branch.
- `MEASURED`: workspace check `cargo +1.81 check --workspace` is blocked on this seat by missing MSVC `link.exe` while compiling dependency build scripts (`proc-macro2`, `quote`).
- `MEASURED`: `cargo test -p asolaria-server-fischer-eval` is blocked on this seat by missing MSVC `link.exe`.
- `UNVERIFIED`: executable runtime smoke is pending a build seat with a linker.

## What Was Ported

The Rust crate mirrors the `BHFISCHER-KERNEL-v1` evaluator shape from `fischer-kernel.mjs`:

- Pipeline role: `VERIFY -> FISCHER-EVAL -> HOOKWALL -> ROUTE`.
- Verdicts: `PROCEED`, `HOLD`, `ANALYZE`, `BLOCK`, `REFUTE`.
- Tier 0: G4 GLSM `MISTAKE_FLAGGED` hard-block.
- Tier 1: illegal envelope hard-block (`missing_pid`, `missing_verb`).
- Tier 2: refuted patterns hard-refute (`self_authorize`, `bypass_hookwall`, `recursive_consent`, `json=true`, authority jump without cosign).
- Tier 3: CPL penalties/gains with hard floors.
- Output: `FISCHERv1|...|json=0|runtime=0|row_hash=...` HBP rows.
- Safety: no self-authorization and no cosign append.

## Acer Cross-Seat Attack-Verify

`MEASURED`: Acer relayed PR comment `#issuecomment-4790181614`, identifying two clippy blockers that
Liris could not see because Liris lacks the MSVC linker. This branch fixes both:

- `clippy::too_many_arguments` on the hard-gate return path: replaced with `HardEvalSpec`.
- `clippy::needless_range_loop` in the inline SHA-256 loader: rewrote the first 16-word load with
  `iter_mut().take(16).enumerate()`.

## Cross-Level Placement

`CANON/OPERATOR_OBSERVED`: Fischer is not a single hierarchy rank. It can sit at OP, council,
supervisor, agent, route, and omni-system levels as a recurring evaluator/blunder gate.

This receipt is therefore only one migration cell: the Rust Host-8 evaluator cell. The broader map
still must carry Hilbra, Atlas Recall, omni systems, registration office sectors, construction yard,
remote-control agents, Brown-Hilbert cubes, N-prime expansion, and storage/NotebookLM staging as
separate migration surfaces.

## Fabric Ask

`MEASURED`: the fabric/council query was attempted for the expanded migration scope and returned
`ok=false` with fallback `all_bases_unavailable` / cooldown. No cube-feed or council ratification is
claimed from this branch.
