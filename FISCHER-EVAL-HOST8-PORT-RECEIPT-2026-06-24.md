# Fischer Eval Host-8 Port Receipt — Liris Side

**Date:** 2026-06-24  
**Branch intent:** additive Rust Host-8 migration build, no live cutover.

## Scope

This branch adds `servers/fischer-eval`, a Rust Host-8 port of the live Fischer evaluator kernel.
It is not a replacement of the live Node runtime yet.

- `MEASURED`: live Fischer remains reachable at `127.0.0.1:4794` and reports `FISCHER-LIVE|ok=1|...|json=0`.
- `MEASURED`: Node ground-truth source lives under `C:\Users\rayss\_bigpickle_acer_fischer\src\`.
- `MEASURED`: Node ground-truth unit suite passes `26/26`.
- `MEASURED`: Rust `cargo check -p asolaria-server-fischer-eval --tests` passes.
- `MEASURED`: static scan of the Rust source found no JSON hot-path markers.
- `MEASURED`: `cargo test -p asolaria-server-fischer-eval` is blocked on this seat by missing MSVC `link.exe`.
- `MEASURED`: `cargo fmt` is blocked on this seat by missing `rustfmt` component.
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

