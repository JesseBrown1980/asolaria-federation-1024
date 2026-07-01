# Host8 replay-prep receipt — task #26 (liris prep, 2026-06-22)

Branch: `liris/host8-replay-prep-26`
Base: `origin/liris/host8-shadow-parity-25` (`8a277cd`)

## What changed

Adds one dry route to `servers/host8-serve/src/main.rs`:

```text
/replay-prep.hbp?target=<N>&sample=<N>&batch=<N>&device=<d>&score=<q>&risk=<q>
```

Default target is `100000000000` and default batch size is `10000`.

The route samples the #25 shadow parity surface, estimates batch geometry, and emits a readiness receipt:

```text
HOST8REPLAYPREP|target_total=...|batch_size=...|estimated_batches=...|sample_checked=...|sample_matched=...|sample_mismatched=...|status=...|packet_registry_loaded=0|shadow_only=1|process_launch=0|auto_fire_allowed=0|operator_t0_required=1|json=0
```

## Safety boundary

- No `fire=1`.
- No `Command::spawn`.
- No room rename.
- No substrate write.
- No 100B run.
- No packet registry is loaded yet.
- `auto_fire_allowed=0` even when the sample is clean.
- Output is HBP tuple text, `json=0`.

## Status meanings

- `READY_FOR_PACKET_REGISTRY_REPLAY`: sampled shadow identities matched; real packet registry replay can be prepared next, still dry.
- `BLOCKED_SHADOW_MISMATCH`: shadow and launch-plan identity disagreed; stop.
- `BLOCKED_NO_SEATS`: seatbook was empty or sample resolved to zero.

## Liris verification

LIRIS_MEASURED:

- branch cut from #25.
- static scan confirms:
  - `/replay-prep.hbp`
  - `HOST8REPLAYPREP`
  - `target_total=100000000000`
  - `packet_registry_loaded=0`
  - `process_launch=0`
  - `auto_fire_allowed=0`
  - `operator_t0_required=1`
  - `json=0`
- static spawn/write scan found no new launch/write primitives in the replay-prep route. Existing hits belong to the pre-existing PID file behavior and old gated summon fire path.

LIRIS_BLOCKED:

- Liris lacks the full Acer MSVC link environment. Final compile/test belongs on Acer.

## Acer next check

Run:

```text
cargo test -p asolaria-host8-serve
```

Expected:

- previous 22 tests still pass.
- 2 new replay-prep tests pass.
- expected total: 24 host8-serve tests, 0 failed.

If green, #26 moves to the next subgate: packet-registry replay loader, still dry and still `auto_fire_allowed=0`.
