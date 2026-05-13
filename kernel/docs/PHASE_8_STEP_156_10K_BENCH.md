# Phase-8 Step 156 · 10K-Envelope Federation Throughput Bench

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 8 · Step 156 (ship-gate dependency for Phase-10 v1.0.0)
**Authored:** 2026-05-11 by Phase-10-Bench (acer-claude scout role)
**Author authority:** standing two-week auth `:82646` (through 2026-05-25)
**Source of pass criteria:** `kernel/docs/PHASE_10_SHIP_CHECKLIST.md §3 "Step 156"`
**Harness path:** `tools/bench/envelope-10k-bench.mjs`
**Result envelope path (at run time):** `xe-execute-2026-05-11/PHASE_10_STEP_156_BENCH_RESULT.behcs-256.json`
**Status:** SPEC ONLY · not yet executed · bus must be confirmed live + warm before any run

---

## 1. Purpose

Establish empirical steady-state throughput + tail-latency ceiling for the
BEHCS-1024 federation bus under a 10,000-envelope soak. Output is one of the
two ship-blocking drills (alongside step 155) that gate the `v1.0.0` tag.

The bench answers three questions:
1. Can the bus sustain ≥ 200 envelopes/sec end-to-end without back-pressure
   collapse? (steady-state ceiling)
2. Is p99 round-trip latency under 25 ms while the bus is hot? (tail behavior)
3. Does the envelope archive leak memory across 10K writes? (memory soak)

---

## 2. Target metrics (mirror of ship-checklist §3 row "Step 156")

| Metric | Target | Source |
|---|---|---|
| Sustained envelope rate (lane-1, acer↔liris 1 Gbps) | **≥ 200 env/sec** | checklist §3 |
| p50 round-trip latency (acer → liris → acer) | **≤ 8 ms** | checklist §3 |
| p95 round-trip latency | **≤ 15 ms** (derived; midpoint p50/p99) | this spec |
| p99 round-trip latency under 10K soak | **≤ 25 ms** | checklist §3 |
| Cosign-chain row-append p99 | **≤ 15 ms** | checklist §3 + Phase-8 invariant 1 |
| Memory growth (acer process RSS) during bench | **≤ 50 MB** | checklist §3 |
| Memory growth (liris process RSS) during bench | **≤ 50 MB** | checklist §3 |
| Hookwall verdict-emit cadence | **≥ 1× per syscall**; ledger appends ≥ 99% | checklist §3 |
| Bilateral parity post-bench | acer + liris cosign-chain row-counts diff ≤ 1 | Phase-8 invariant |

If **any** of envelope-rate / p99-latency / memory-growth fails, step 156 fails
and v1.0.0 punts to v1.0.0-rc.2.

---

## 3. Test corpus shape

Per checklist §3 row "Test-corpus envelope shape" — the 10K corpus exercises
the 5-subclass classifier under load:

| Subclass mix | Count | Shape |
|---|---|---|
| `Regular` L0 verdicts | 6,000 (60%) | `<ROLE>-PID-<REGION><HOST>-A##-W###` |
| `RegularExtended` | 3,000 (30%) | `…-A##-W###-P##-N#####` (process+nonce suffix) |
| `Anchor` | 1,000 (10%) | `<ROLE>-PID-<YYYY-MM-DD>` |

Per-envelope size target: **500–1000 bytes** (realistic federation shape;
matches checklist §3 note "realistic shape ~500-1000 byte envelopes").
The minimal-200B template in the harness skeleton is a **dry-run lower bound**;
production-shape padding must be added before the recorded bench.

---

## 4. Test method

### 4.1 Single-vantage POST loop (this harness — `envelope-10k-bench.mjs`)

- Driver: acer-claude process via `tools/bench/envelope-10k-bench.mjs`.
- Target: real bus `http://192.168.1.50:4947/behcs/send` (canonical bus host
  per `reference_liris_hold_posture_bus_canonical_host.md`).
- Concurrency model: configurable parallelism (default 8 in-flight); sequential
  loop available via `--concurrency=1`.
- Per-request measurement: `performance.now()` straddling the `fetch` call;
  recorded into a flat `Float64Array` for histogram + percentile pass.
- Output: a single result envelope summarizing rate + p50/p95/p99/max +
  memory-growth-window + an inline histogram (32 log-spaced buckets).

This single-vantage method measures **bus ingest latency** (POST → 2xx ack).
It does **not** measure full acer→liris→acer round-trip; for the full RTT
metric the bench must be re-run in cross-vantage mode (4.2).

### 4.2 Cross-vantage distributed (future / step 156-RTT)

- Driver A: acer-claude bench POSTs N envelopes destined for liris.
- Driver B: liris-claude bench echoes each envelope back to acer.
- Driver A records "send timestamp" and "echo-received timestamp"; the delta is
  full round-trip latency.
- This is the variant that **directly answers checklist §3 p50/p99 RTT rows**.
- This spec authorizes the cross-vantage variant but the harness skeleton in
  this commit is the single-vantage form only (smaller blast radius; safe to
  author without coordinating a 2-vantage launch).

### 4.3 Sequence

1. Pre-flight: confirm bus `/health` returns 200 + cosign-chain reachable.
2. Warm-up: 100 envelopes, results discarded.
3. Soak: 10,000 envelopes, all results recorded.
4. Settle: 30 s idle, then re-probe `/health` + cosign-chain row count on
   both vantages (parity check).
5. Emit result envelope.

---

## 5. Measurement method

| Sample | Captured by | Stored as |
|---|---|---|
| Per-request latency (ms) | `performance.now()` before/after `fetch` | `Float64Array(10000)` |
| HTTP status code | `response.status` | counter map (`200`, `5xx`, `timeout`) |
| Driver process RSS at bench-start | `process.memoryUsage().rss` | bytes |
| Driver process RSS at bench-end | `process.memoryUsage().rss` | bytes |
| Bus RSS pre/post | manual via liris/acer SSH; out of scope of harness | logged in result envelope |
| Cosign-chain row count pre/post | manual via `wc -l COSIGN_CHAIN.ndjson` on both vantages | logged in result envelope |

Percentiles computed by sort-and-index on the latency array
(`samples.sort(); p99 = samples[Math.floor(0.99 * n)]`). The harness emits
p50/p95/p99/max as raw numbers; the result envelope is the authoritative
artifact for the checklist gate.

---

## 6. Pass / fail criteria (gate semantics)

Step 156 PASSES iff **all** of the following hold:

1. Throughput: `total_envelopes / wall_seconds ≥ 200`.
2. p50 latency ≤ 8 ms (single-vantage form: bus-ingest only; cross-vantage form:
   full RTT).
3. p99 latency ≤ 25 ms.
4. HTTP error rate < 1% (i.e. ≥ 9,900 of 10,000 returned 2xx).
5. Driver RSS growth ≤ 50 MB; bus RSS growth ≤ 50 MB on each vantage.
6. Bilateral parity: post-settle cosign-chain row-count diff between acer and
   liris ≤ 1.
7. Hookwall verdict-emit cadence ≥ 99% (recorded by bus, cross-checked
   post-bench).

Any single criterion failing → step 156 FAIL → v1.0.0 punts to rc.2.

---

## 7. Operational guard-rails

- **Do not run** without operator authorization on the day — 10K envelopes in
  rapid succession will spike bus load and may trigger hookwall back-pressure
  in other live sessions.
- Run only inside the standing two-week auth window `:82646` (through
  2026-05-25).
- Pre-coordinate with liris vantage so the cosign-chain append path is warm
  and the bilateral-parity probe will succeed.
- Abort condition: if HTTP 5xx rate exceeds 5% in any 100-envelope window,
  the harness should stop and emit a partial-result envelope rather than
  continue and damage the chain.

---

## 8. Open items

- [ ] Cross-vantage variant (§4.2) script — co-author with liris-claude.
- [ ] Bus `/health` endpoint confirmation — verify the exact path before run.
- [ ] Production-shape 500–1000B padding — harness ships with minimal 200B
      template; pad before the recorded bench.
- [ ] Result-envelope schema — `xe-execute-2026-05-11/PHASE_10_STEP_156_BENCH_RESULT.behcs-256.json`
      shape to be defined alongside step-155 result envelope for consistency.
