# GNN Architecture

**Anchor PID:** `ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11`
**Phase:** 4 - Step 61
**Status:** v0.1 architecture contract
**Primary crate:** `servers/gnn-oracle`
**Kernel compatibility shim:** `kernel/core/src/gnn`

## Purpose

The GNN lane is the inference primitive required by `REPO_LAW.md`
Invariant 3. It supplies three decisions:

- route prediction for BEHCS-1024 envelopes
- top-N ranking for dashboard and fabric candidates
- supervisor verdict aggregation

The kernel keeps only the syscall-facing compatibility shim. The real model
runtime belongs in `servers/gnn-oracle`, with kernel access later routed through
envelope IPC.

## Graph Shape

Phase-4 uses a graph attention network over the live federation graph.

| Component | Contract |
|---|---|
| Target edge count | `2_158_671` directed edges |
| Node families | envelope, PID, agent role, device vantage, hookwall verdict, cosign row, tier, route |
| Edge families | emitted-by, addressed-to, cosigned-by, routed-to, blocked-by, ranked-before, supersedes |
| Model class | graph attention network with typed edge embeddings |
| Primary inference mode | userspace server call from `servers/gnn-oracle` |
| Kernel fallback | deterministic local result when model unavailable |

The current scaffold already exposes the target count as
`GNN_EDGES_TARGET = 2_158_671` in both the userspace oracle and kernel shim.

## Feature Schema

Feature extraction must be deterministic and reproducible from append-only
artifacts. The v0.1 contract uses fixed-width records so the ONNX export and
later no_std-friendly runtime can share one shape.

| Feature group | Required fields |
|---|---|
| Envelope | type id, source PID sha16, destination PID sha16, tier, payload length bucket |
| PID | role class, device class, lane class, mint prime bucket |
| Hookwall | pre verdict, post verdict, hold/block reason bucket, latency bucket |
| Cosign | row number, signer quorum class, previous-row sha16, content sha16 |
| Route | previous route class, observed destination, success/failure class |
| Time | monotonic row window, coarse freshness bucket |

No raw secret, hidden payload, or unrestricted content is a model feature.
Restricted inputs must be represented as hashes, labels, or redacted buckets.

## Outputs

### Routing

`predict_route(envelope_bytes)` returns one `RoutingDecision`:

| Code | Decision |
|---:|---|
| 0 | `Local` |
| 1 | `TriadBroadcast` |
| 2 | `QuadBroadcast` |
| 3 | `UnicastAcer` |
| 4 | `UnicastLiris` |
| 5 | `UnicastFalcon` |
| 6 | `UnicastAether` |

The kernel syscall ABI encodes the same decision as a single byte.

### Ranking

`rank_top_n(items, n)` returns stable item ids and scores. When the model is
unavailable, the deterministic fallback preserves input order and assigns
monotonically decreasing scores.

### Verdict Aggregation

`aggregate_verdict(votes_in_favor, total_votes)` returns:

- `ProceedStrong`
- `ProceedWeak`
- `Hold`
- `Block`
- `Investigate`

The v0.1 unavailable-model fallback is `Hold`, which is the conservative
fail-closed result.

## Runtime Boundary

`servers/gnn-oracle` is the owner for model loading, inference, model-swap
authorization, version metadata, and observability. `kernel/core/src/gnn` exists
only to keep Phase-3 syscall tests and the current ABI stable until the
microkernel demotion replaces direct calls with envelope IPC.

Hard boundaries:

- kernel code stays `#![forbid(unsafe_code)]`
- model loading requires cosign-gated update protocol before becoming live
- deterministic fallback is mandatory whenever the model is unavailable
- silent fail-open routing is forbidden
- p99 inference budget is `100 ms`
- batch target is `40_783/sec` sustained for the Phase-4 benchmark lane

## Versioning

Every model artifact must have a PID in this form:

```text
GNN-MODEL-PID-H<sha16>-v<n>
```

The version ledger for Phase-4 step 78 is `GNN_VERSIONS.ndjson`. Each row must
record model PID, sha16, parent model PID, training dataset hash, export hash,
cosign quorum, benchmark summary, and activation status.

## Step Mapping

| Phase-4 step | Architecture implication |
|---:|---|
| 61 | This document defines the GAT contract and 2.16M-edge target |
| 62 | Training data comes from sha16-indexed cosign-chain history |
| 63 | ONNX export must load under onnxruntime before promotion |
| 64 | Userspace inference lands in `servers/gnn-oracle` |
| 65 | Ranking adapter must match reference top-K |
| 66 | `/api/gnn/topN` v2 must expose BEHCS-1024 anchored results |
| 67 | Bus routing oracle consumes envelopes and emits route prediction |
| 68 | Verdict aggregator consumes supervisor vote summaries |
| 69 | Hookwall may consult GNN but must preserve fail-closed behavior |
| 70 | Benchmark p99 must stay under `100 ms` |
| 71 | Sustained batch target is `40_783/sec` for 30 seconds |
| 72 | Model sidecar carries BEHCS-1024 metadata |
| 73 | Unauthorized model swaps return BLOCK |
| 74 | Model PID versioning is mandatory |
| 75 | Known-answer suite must reach at least 95 percent correctness |
| 76 | Failover test proves deterministic fallback |
| 77 | Metrics report count, p50, p99, and error rate |
| 78 | Version rows append to `GNN_VERSIONS.ndjson` |
| 79 | Benchmark evidence lands in `BENCHMARK_GNN.md` |
| 80 | Tier landed envelope is emitted only after steps 61-79 verify |
