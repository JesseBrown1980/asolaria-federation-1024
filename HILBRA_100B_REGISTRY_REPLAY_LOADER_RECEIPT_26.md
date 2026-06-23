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

## Output

The route still emits the existing `HOST8REPLAYPREP` dry gate line, then adds:

```text
HOST8REGISTRY|loaded=...|checkpoint_processed=...|checkpoint_target=...|chunks_loaded=...|packets_loaded=...|avg_score_q=...|avg_reverse_gain_q=...|reverse_risk_q=...|promote_chunks=...|hold_chunks=...|registry_status=...|reverse_risk_mapping=one_minus_reverse_gain|process_launch=0|auto_fire_allowed=0|json=0
```

No raw packet bodies are emitted. The path is represented only by `path_sha16`.

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

## Acer verification required

Run on Acer/MSVC against the real registry:

```text
cargo test -p asolaria-host8-serve
GET /replay-prep.hbp?...&registry=C:/Users/acer/Asolaria/data/neurotech-defense-lab/real-agents/100b-run/
```

Expected real-run signal if the registry matches the scouted shape:

- checkpoint processed/target: `100000000000`
- chunks: `100000`
- packets loaded: `100000000000`
- reverse risk around `225` when avgReverseGain is about `0.775`
- `auto_fire_allowed=0`
- `process_launch=0`

