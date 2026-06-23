# Host8 shadow-parity receipt — task #25 (liris prep, 2026-06-22)

Branch: `liris/host8-shadow-parity-25`
Base: `origin/acer/host8-launch-plan-24` (`f5301cf`)

## What changed

Adds one dry route to `servers/host8-serve/src/main.rs`:

```text
/shadow-parity.hbp?count=<N>&device=<d>&role=<hermes|sub>&score=<q>&risk=<q>
```

It replays two dry paths side by side:

1. `/summon.hbp` resolve-only path, with no `fire=1` parameter.
2. `/launch-plan.hbp` dry planner path from #24.

For each checked seat it emits:

```text
HOST8SHADOWROW|handle8=...|summon_ok=...|plan_ok=...|summon_pid=...|plan_pid=...|gate_verdict=...|summon_fired=0|process_launch=0|parity=OK|json=0
```

And a summary:

```text
HOST8SHADOW|requested=...|checked=...|matched=...|mismatched=...|proceed=...|hold=...|block=...|process_launch=0|fire_param=absent|json=0
```

## Safety boundary

- No `fire=1`.
- No `Command::spawn`.
- No room rename.
- No substrate write.
- No 100B run.
- No live service restart.
- Output is HBP tuple text, `json=0`.

## Why this matters

This is the replay surface for #26.

Before any controlled 100B run, the system needs to prove that the old resolve-only summon identity and the new launch-plan gate agree on the exact `instance_pid` while keeping all live actions dry. This route produces that proof in batch form.

## Liris verification

LIRIS_MEASURED:

- branch cut from `origin/acer/host8-launch-plan-24`
- static route/function/test scan confirms:
  - `/shadow-parity.hbp`
  - `HOST8SHADOW`
  - `HOST8SHADOWROW`
  - `process_launch=0`
  - `fire_param=absent`
  - `json=0`
- secret scan found no key values.

LIRIS_BLOCKED:

- `rustfmt` component is not installed for the local `stable-x86_64-pc-windows-msvc` toolchain.
- Liris still lacks MSVC `link.exe`, so final compile/test must run on Acer.

## Acer next check

Run:

```text
cargo test -p asolaria-host8-serve
```

Expected:

- existing 20 tests from #24 still pass
- 2 new shadow-parity tests pass
- expected total: 22 host8-serve tests, 0 failed

If green, #25 becomes `MEASURED_GREEN` and #26 replay-prep can begin.
