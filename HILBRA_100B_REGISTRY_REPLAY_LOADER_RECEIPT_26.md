# Hilbra 100B registry replay-loader receipt - task #26 continuation

Branch: `liris/host8-100b-registry-loader-26`
Base: `origin/main` after dry stack merge (`13f8063`)

## What changed

`servers/host8-serve/src/main.rs` extends the dry `/replay-prep.hbp` route with an optional
read-only registry loader:

```text
/replay-prep.hbp?target=<N>&sample=<N>&batch=<N>&device=<d>&score=<q>&risk=<q>&registry=<dir>&chunk_limit=<N>
```

When `registry=<dir>` is supplied, the route reads:

- `<dir>/checkpoint.state.json`
- `<dir>/real-100b-chunks.ndjson`

It streams chunk rows, computes aggregate packet counts, genius/mistake totals, weighted
`avgScore`, weighted `avgReverseGain`, and derives:

```text
reverse_risk = 1 - avgReverseGain
```

That maps the Acer raw-disk omniflywheel canon ("validated-state recirculation into the next
wave") onto the Rust gate contract:

- `WHITEROOM_GENIUS_THRESHOLD = 0.72`
- `OMNIFLYWHEEL_MAX_REVERSE_RISK = 0.28`
- spawn gate quantized parity: `score_q >= 720 && reverse_risk_q <= 280`

The loader accepts both registry chunk modes:

- `real_100b_chunk` (full-detail early/window chunks)
- `real_100b_accelerated_chunk` (bulk `chunk_aggregate_sparse_proof` chunks)

This preserves the 2000-resident-bound / gulp / sparse-proof architecture: the first roughly
2000 full-detail chunks and the accelerated bulk together form the 100,000-chunk / 100B-packet
run. Filtering only `real_100b_chunk` is a deflation bug.

## Output

The route still emits the existing `HOST8REPLAYPREP` dry gate line, then adds:

```text
HOST8REGISTRY|loaded=...|checkpoint_processed=...|checkpoint_target=...|chunks_loaded=...|real_chunks=...|accelerated_chunks=...|packets_loaded=...|avg_score_q=...|avg_reverse_gain_q=...|reverse_risk_q=...|promote_chunks=...|hold_chunks=...|scalar_gate_clean=...|registry_status=...|reverse_risk_mapping=one_minus_reverse_gain|accepted_chunk_kinds=real_100b_chunk,real_100b_accelerated_chunk|process_launch=0|auto_fire_allowed=0|json=0
HOST8PIPELINE|required=gnn,hookwall,reverse_gain_gnn,whiteroom,omnishannon,shannon,omniflywheel|scalar_projection=1|pipeline_verified=0|status=SUPERVISOR_PIPELINE_REQUIRED|process_launch=0|auto_fire_allowed=0|json=0
```

No raw packet bodies are emitted. The path is represented only by `path_sha16`.

Important: `reverse_risk = 1 - avgReverseGain` is only the scalar projection into the Rust gate.
It does not replace the required supervisor pipeline: forward GNN, hookwall, reverse-gain GNN,
WhiteRoom, OmniShannon, Shannon, and Omniflywheel recirculation. A clean aggregate can only report
`SCALAR_GATE_READY_SUPERVISOR_PIPELINE_REQUIRED`, not live-run readiness.

## Safety boundary

- Read-only registry access.
- No substrate writes.
- No process spawn.
- `process_launch=0` always.
- `auto_fire_allowed=0` always.
- Operator T0 still required for any live run.

## Liris verification

- `git diff --check`: pass.
- HBP route strings and tests present by static inspection.
- `cargo fmt`: not available on Liris (`rustfmt` component missing).
- `cargo check` / `cargo test`: blocked on Liris because MSVC `link.exe` is not installed.

## Acer verification

Initial required command set:

```text
cargo test -p asolaria-host8-serve
GET /replay-prep.hbp?...&registry=C:/Users/acer/Asolaria/data/neurotech-defense-lab/real-agents/100b-run/
```

OPERATOR_OBSERVED / Acer-measured follow-up on the PR #6 worktree:

- `cargo test -p asolaria-host8-serve`: `26 passed; 0 failed` after the rounding-boundary fix.
- The fixture's weighted `avgReverseGain = 0.7475` resolves as `reverse_risk_q=252` on Acer/MSVC
  (`1 - 0.7475` becomes `0.2524999..` in f64, then `.round()` -> `252`). This supersedes the
  hand-computed `253` expectation.
- Acer ground-truth oracle over the real registry:
  - chunks: `100000`
  - `real_100b_chunk = 1968`
  - `real_100b_accelerated_chunk = 98032`
  - other chunk kinds: `0`
  - packets: `100000000000`
  - aggregate chunk gate: `PROCEED` for `100000`, `HOLD` for `0`
  - checkpoint cross-checks match exactly:
    - `sum_packets = 100000000000`
    - `sum_genius = 277800007`
    - `sum_mistake = 111103104`
- Honest nuance from proof samples: packet-level samples include both `PROCEED` and `HOLD`
  (`10/15` proceed, `5/15` hold in the cited Acer run). Held packets are compacted/never-deleted;
  they are not spawned. Aggregate chunk readiness must not erase packet-level variance.

Still true after Acer proof:

- `auto_fire_allowed=0`
- `process_launch=0`
- `HOST8PIPELINE|...|pipeline_verified=0|status=SUPERVISOR_PIPELINE_REQUIRED`
