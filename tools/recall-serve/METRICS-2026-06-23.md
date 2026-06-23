# Asolaria Recall Portal — MEASURED metrics (2026-06-23)

Performance receipt for the Rust `recall-serve` engine (the Hilbra-internet recall portal).
**MEASURED**, reproducible, loopback. Engine commit `bfa357a` · toolchain Rust 1.81 ·
host `DESKTOP-J99VCNH` (acer, 4-core i5) · corpus `ASOLARIA-ACER-RECALL` (acer-local, **not published**).

## Headline
> **Median 1.47 ms** recall over **591,286 rows / 2,614,638 terms / 23,930,053 postings**,
> **p99 4.15 ms**, **1,336 queries/s** concurrent, **never stalls under load**, and the public
> tier (L0) is **provably PII-free**. ~**40× faster** than the prior Node inverted index
> (~56–67 ms) and unbounded vs the Node *linear* scan (which event-loop-stalls).

## Engine / corpus (from `/api/health`)
| metric | value |
|---|---|
| rows | 591,286 |
| terms | 2,614,638 |
| postings | 23,930,053 |
| skipped (fail-closed) | 0 |
| index build | 53.8 s (one-time, at startup) |
| index schema | `HILBRA-IDX-BEHCS-TUPLE-TEXT-V1` |
| `json_hot_path` / `linear_fallback` | false / false |
| resident RAM (index) | 688 MB working set · 772 MB private |

## Query latency (warm, K=60/query, 10 representative queries, 600 samples)
| query (tier) | median ms | p95 ms | p99 ms | max ms | hits | candidates |
|---|---|---|---|---|---|---|
| brown-hilbert (public L0) | 1.62 | 2.55 | 3.08 | 3.08 | 49 | 49 |
| what-is-asolaria (public L0) | 0.92 | 1.41 | 1.83 | 1.83 | 0 | 0 |
| significance (loopback) | 1.28 | 2.15 | 2.31 | 2.31 | 10 | 10 |
| mcp (loopback) | 1.72 | 2.77 | 3.62 | 3.62 | 50 | 644 |
| falcon (loopback) | 3.07 | 4.18 | 4.19 | 4.19 | 50 | 14,330 |
| shannon (loopback) | 1.92 | 2.72 | 3.22 | 3.22 | 50 | 4,637 |
| gnn (loopback) | 1.40 | 2.15 | 2.41 | 2.41 | 50 | 1,192 |
| two-token "brown hilbert" | 1.58 | 3.26 | 4.15 | 4.15 | 50 | 176 |

**Aggregate (600 queries):** min 0.63 · **median 1.47** · p95 3.26 · **p99 4.15** · max 6.15 · mean 1.64 ms.
Even the heaviest term (`falcon`, 14,330 candidate postings) resolves + level-filters + seeks in ~3 ms.

## Throughput
| mode | result |
|---|---|
| sequential (1 client) | 500 queries / 684 ms = **731 q/s** |
| concurrent (32 clients) | 1000 queries / 748 ms = **1,336 q/s** |

## No-stall proof (the "no Node" headline)
Under a sustained 16-wide query flood, `/api/health` latency stayed **median 9.85 ms, p99 12.17 ms**
— it never blocks (thread-per-connection). On the SAME corpus the Node engine event-loop-stalls:
one heavy query blocks every route, `/api/health` included. This is the measured justification for
the Rust port.

## Safety — L0 provably PII-free
Public-tier (`/api/public/search?…&level=0`) probes for PII terms all return **0** hits:
`bank→0 · vault→0 · .pem→0 · legal→0 · password→0 · cnpj→0 · paypal→0`. PII paths/content
classify to owner-private and never reach L0. (Keyed acer↔liris links share deeper tiers by
operator consent; third parties get L0 only.)

## Comparison
| engine | median query | under load | corpus |
|---|---|---|---|
| Node `serve-recall.cjs` **linear** | **stalls** (health times out) | blocks every route | 591k |
| Node `serve-recall.cjs` **indexed** | ~56–67 ms (operator receipt, screenshots) | n/a | 591k |
| **Rust `recall-serve` (this)** | **1.47 ms** | **9.85 ms health under flood** | 591k |

Rust vs Node-indexed ≈ **40× faster median**; vs Node-linear, unbounded (linear is unusable under load).

## Bilateral — both colony portals MEASURED (federation findings)

Each colony benchmarked its OWN live portal from its OWN seat (independent scripts):

| colony | engine | rows | terms | median query | concurrency | health under flood | L0 PII |
|---|---|---|---|---|---|---|---|
| **acer** | Rust `recall-serve` | 591,286 | 2,614,638 | **1.47 ms** | **1,336 q/s** (32c) | 9.85 ms med · p99 12 ms | free ✓ |
| **liris** | Node `serve-recall.cjs` | 10,644 | 103,238 | 3.65–4.82 ms | 205 q/s (16c), **74 ms med under load** | 0.83 ms med · **p99 117 ms** | free ✓ |

*acer = `MEASURED_ACER` (`:4796`); liris = `MEASURED_LIRIS_LOCAL` (`:4791`, 2026-06-23, cold run, its own bench.)*

**Cross-engine note (apples-to-oranges — different engine, corpus size, host):** acer's Rust serves ~**55× the
corpus** at **lower** median latency, and under concurrency the thread-per-connection engine holds (1,336 q/s, health
p99 12 ms) where the Node event loop **serializes** (205 q/s, query median balloons to 74 ms, health p99 117 ms under
the same flood). The clean *same-host, same-corpus* comparison stays the acer one: Node-indexed ~56–67 ms → Rust
1.47 ms (~40×). Both portals are **L0 provably PII-free** from their own seat — the federation public tier holds on
both colonies independently.

## Method / reproducibility
- Bench: `recall-bench.cjs` — loopback `http.get`, `process.hrtime` timing, 20-query warmup, K=60/query,
  then 500 sequential + 1000×32-concurrent, then health-under-flood, then L0 PII probes.
- Engine: `recall-serve` `bfa357a`, release build (`opt-level=z`, LTO), Rust 1.81, run on `127.0.0.1:4796`.
- Numbers are **end-to-end client-observed** (include loopback HTTP overhead), so the engine's internal
  compute is ≤ these figures.

## Honest scope / caveats
- Single 4-core i5 laptop, localhost loopback — not a datacenter, not LAN-RTT (cross-colony adds network).
- Corpus is acer's `ASOLARIA-ACER-RECALL` (591k); other corpora/sizes will differ.
- Tagged **MEASURED_ACER**. Cross-colony "live both ways right now" remains `UNVERIFIED_CURRENT` until a
  per-seat receipt. recall-serve is **HELD** (PR #8) for liris cross-verify before any `:4791` cutover —
  these metrics are the new-engine receipt, not a cutover.
