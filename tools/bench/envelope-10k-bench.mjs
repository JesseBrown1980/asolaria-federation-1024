#!/usr/bin/env node
// envelope-10k-bench.mjs
//
// Phase-8 step 156 · 10K envelope federation throughput bench (SKELETON).
//
// Spec: kernel/docs/PHASE_8_STEP_156_10K_BENCH.md
// Pass criteria: kernel/docs/PHASE_10_SHIP_CHECKLIST.md §3 row "Step 156".
//
// Single-vantage POST loop form: this harness only measures bus-ingest latency
// (POST -> 2xx ack from canonical bus host). Cross-vantage full-RTT variant
// is spec'd in §4.2 of the spec doc and authored separately.
//
// Authority: standing two-week auth :82646 (through 2026-05-25).
//
// SAFETY: this skeleton MUST NOT be run casually. 10K rapid POSTs against the
// live bus will spike load and may trigger hookwall back-pressure in other
// sessions. Operator authorization required on the day. Default --dry-run
// prevents accidental execution.
//
// Usage:
//   node tools/bench/envelope-10k-bench.mjs --dry-run            # default; emits plan only
//   node tools/bench/envelope-10k-bench.mjs --execute             # actually runs the bench
//   node tools/bench/envelope-10k-bench.mjs --execute -n 10000 --concurrency 8
//   node tools/bench/envelope-10k-bench.mjs --execute --url http://192.168.1.50:4947/behcs/send
//
// Exit codes:
//   0 = bench complete OR dry-run plan emitted
//   1 = bench aborted (5xx rate breach) OR pre-flight failed
//   2 = invalid args

'use strict';

import { performance } from 'node:perf_hooks';
import { argv, exit, memoryUsage, hrtime } from 'node:process';
import { writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

// ---------------------------------------------------------------------------
// Constants from spec doc §2-§5
// ---------------------------------------------------------------------------

const DEFAULT_BUS_URL = 'http://192.168.1.50:4947/behcs/send';
const DEFAULT_N = 10000;
const DEFAULT_WARMUP = 100;
const DEFAULT_CONCURRENCY = 8;
const DEFAULT_TIMEOUT_MS = 5000;
const ABORT_5XX_WINDOW = 100;          // sliding window size
const ABORT_5XX_RATE = 0.05;           // 5% in window -> abort
const BENCH_TAG = 'phase-8-step-156';

// Pass-criteria thresholds (mirrored from spec doc §6)
const PASS = {
  minThroughputEnvPerSec: 200,
  p50LatencyMs: 8,
  p95LatencyMs: 15,
  p99LatencyMs: 25,
  maxErrorRate: 0.01,
  maxRssGrowthMb: 50,
};

// ---------------------------------------------------------------------------
// CLI parsing (minimal, no deps)
// ---------------------------------------------------------------------------

function parseArgs(args) {
  const opts = {
    url: DEFAULT_BUS_URL,
    n: DEFAULT_N,
    warmup: DEFAULT_WARMUP,
    concurrency: DEFAULT_CONCURRENCY,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    execute: false,
    outPath: null,
  };
  for (let i = 2; i < args.length; i++) {
    const a = args[i];
    switch (a) {
      case '--dry-run': opts.execute = false; break;
      case '--execute': opts.execute = true; break;
      case '--url':     opts.url = args[++i]; break;
      case '-n':
      case '--count':   opts.n = Number(args[++i]); break;
      case '--warmup':  opts.warmup = Number(args[++i]); break;
      case '--concurrency':
      case '-c':        opts.concurrency = Number(args[++i]); break;
      case '--timeout-ms': opts.timeoutMs = Number(args[++i]); break;
      case '--out':     opts.outPath = args[++i]; break;
      case '--help':
      case '-h':
        printUsage();
        exit(0);
        break;
      default:
        if (a.startsWith('--')) {
          console.error(`unknown arg: ${a}`);
          exit(2);
        }
    }
  }
  if (!Number.isFinite(opts.n) || opts.n <= 0) { console.error('bad --count'); exit(2); }
  if (!Number.isFinite(opts.concurrency) || opts.concurrency <= 0) { console.error('bad --concurrency'); exit(2); }
  return opts;
}

function printUsage() {
  console.log(`envelope-10k-bench.mjs  (Phase-8 step 156)

  --dry-run                  print plan, do not POST (default)
  --execute                  actually POST envelopes to bus
  --url <URL>                bus endpoint (default ${DEFAULT_BUS_URL})
  -n, --count <N>            envelope count (default ${DEFAULT_N})
  --warmup <N>               warmup envelopes, discarded (default ${DEFAULT_WARMUP})
  -c, --concurrency <N>      in-flight requests (default ${DEFAULT_CONCURRENCY})
  --timeout-ms <N>           per-request timeout (default ${DEFAULT_TIMEOUT_MS})
  --out <PATH>               result envelope output path
  -h, --help                 this message
`);
}

// ---------------------------------------------------------------------------
// Envelope template (minimal ~200B; production-shape padding deferred per spec §3)
// ---------------------------------------------------------------------------

function makeEnvelope(seq) {
  // Minimal valid envelope shape. Real production-shape envelopes carry
  // ~500-1000B per spec §3; this minimal form is the dry-run lower bound.
  return {
    pid: `BENCH-PID-PHASE-8-STEP-156-A00-W${String(seq).padStart(3, '0')}`,
    role: 'phase-10-bench',
    ts: new Date().toISOString(),
    payload: { seq, bench: BENCH_TAG },
    cosign: null,
  };
}

// ---------------------------------------------------------------------------
// Pre-flight (spec §4.3 step 1)
// ---------------------------------------------------------------------------

async function preflight(busUrl, timeoutMs) {
  // Probe the bus base origin for liveness. We DO NOT POST during preflight;
  // we just want to know the host is up. A 200/404 from the origin is fine;
  // network error means abort.
  const origin = new URL(busUrl).origin;
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const r = await fetch(origin + '/', { method: 'GET', signal: ctrl.signal });
    clearTimeout(t);
    return { ok: true, status: r.status, origin };
  } catch (err) {
    clearTimeout(t);
    return { ok: false, error: String(err), origin };
  }
}

// ---------------------------------------------------------------------------
// Single-request POST with timeout + latency capture
// ---------------------------------------------------------------------------

async function postOne(url, envelope, timeoutMs) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  const t0 = performance.now();
  let status = 0;
  let errored = false;
  try {
    const r = await fetch(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(envelope),
      signal: ctrl.signal,
    });
    status = r.status;
    // drain body for fairness; ignore content
    try { await r.text(); } catch {}
  } catch {
    errored = true;
  } finally {
    clearTimeout(timer);
  }
  const t1 = performance.now();
  return { latencyMs: t1 - t0, status, errored };
}

// ---------------------------------------------------------------------------
// Bench loop with bounded concurrency + sliding 5xx abort guard
// ---------------------------------------------------------------------------

async function runBench({ url, n, warmup, concurrency, timeoutMs }) {
  // Warm-up (results discarded, but we still respect concurrency)
  await runBatch({ url, count: warmup, concurrency, timeoutMs, latencies: null, statuses: null });

  const latencies = new Float64Array(n);
  const statuses = new Int32Array(n);
  const errors = new Int32Array(n);   // 1 = network error

  // Sliding-window 5xx tracker (spec §7 abort condition)
  const recent5xx = new Array(ABORT_5XX_WINDOW).fill(0);
  let recentIdx = 0;
  let recentSum = 0;
  let aborted = false;
  let aborted_at = -1;

  const wallStart = performance.now();
  const rssStart = memoryUsage().rss;

  let next = 0;
  async function worker() {
    while (true) {
      if (aborted) return;
      const i = next++;
      if (i >= n) return;
      const env = makeEnvelope(i);
      const { latencyMs, status, errored } = await postOne(url, env, timeoutMs);
      latencies[i] = latencyMs;
      statuses[i] = status;
      errors[i] = errored ? 1 : 0;

      // sliding window
      const is5xx = (status >= 500 && status < 600) || errored ? 1 : 0;
      recentSum -= recent5xx[recentIdx];
      recent5xx[recentIdx] = is5xx;
      recentSum += is5xx;
      recentIdx = (recentIdx + 1) % ABORT_5XX_WINDOW;
      if (i >= ABORT_5XX_WINDOW && recentSum / ABORT_5XX_WINDOW > ABORT_5XX_RATE) {
        aborted = true;
        aborted_at = i;
        return;
      }
    }
  }

  await Promise.all(Array.from({ length: concurrency }, () => worker()));

  const wallEnd = performance.now();
  const rssEnd = memoryUsage().rss;

  const done = aborted ? aborted_at + 1 : n;
  return {
    n: done,
    aborted,
    wallSeconds: (wallEnd - wallStart) / 1000,
    rssStart, rssEnd,
    latencies: latencies.slice(0, done),
    statuses: statuses.slice(0, done),
    errors: errors.slice(0, done),
  };
}

async function runBatch({ url, count, concurrency, timeoutMs }) {
  let next = 0;
  async function w() {
    while (true) {
      const i = next++;
      if (i >= count) return;
      await postOne(url, makeEnvelope(-1 - i), timeoutMs);
    }
  }
  await Promise.all(Array.from({ length: concurrency }, () => w()));
}

// ---------------------------------------------------------------------------
// Stats / histogram
// ---------------------------------------------------------------------------

function percentile(sortedArr, p) {
  if (sortedArr.length === 0) return 0;
  const idx = Math.min(sortedArr.length - 1, Math.floor(p * sortedArr.length));
  return sortedArr[idx];
}

function histogram(samples, bucketCount = 32) {
  if (samples.length === 0) return [];
  let min = Infinity, max = -Infinity;
  for (const v of samples) { if (v < min) min = v; if (v > max) max = v; }
  if (min <= 0) min = 0.001;
  const logMin = Math.log(min);
  const logMax = Math.log(Math.max(max, min + 0.001));
  const step = (logMax - logMin) / bucketCount;
  const buckets = new Array(bucketCount).fill(0).map((_, i) => ({
    loMs: Math.exp(logMin + i * step),
    hiMs: Math.exp(logMin + (i + 1) * step),
    count: 0,
  }));
  for (const v of samples) {
    let b = Math.floor((Math.log(Math.max(v, 0.001)) - logMin) / step);
    if (b < 0) b = 0;
    if (b >= bucketCount) b = bucketCount - 1;
    buckets[b].count++;
  }
  return buckets;
}

function summarize(result) {
  const lat = Array.from(result.latencies).sort((a, b) => a - b);
  const errCount = result.errors.reduce((s, v) => s + v, 0);
  let twoXx = 0, fiveXx = 0, other = 0;
  for (const s of result.statuses) {
    if (s >= 200 && s < 300) twoXx++;
    else if (s >= 500 && s < 600) fiveXx++;
    else other++;
  }
  const throughput = result.n / result.wallSeconds;
  const rssGrowthMb = (result.rssEnd - result.rssStart) / (1024 * 1024);

  const p50 = percentile(lat, 0.50);
  const p95 = percentile(lat, 0.95);
  const p99 = percentile(lat, 0.99);
  const max = lat.length ? lat[lat.length - 1] : 0;
  const errorRate = (errCount + fiveXx + other) / Math.max(result.n, 1);

  const passes = {
    throughput: throughput >= PASS.minThroughputEnvPerSec,
    p50:        p50 <= PASS.p50LatencyMs,
    p95:        p95 <= PASS.p95LatencyMs,
    p99:        p99 <= PASS.p99LatencyMs,
    errorRate:  errorRate <= PASS.maxErrorRate,
    rss:        rssGrowthMb <= PASS.maxRssGrowthMb,
  };
  const allPass = Object.values(passes).every(Boolean) && !result.aborted;

  return {
    bench: BENCH_TAG,
    n: result.n,
    aborted: result.aborted,
    wallSeconds: result.wallSeconds,
    throughputEnvPerSec: throughput,
    latency: { p50, p95, p99, max },
    statusCounts: { '2xx': twoXx, '5xx': fiveXx, other, errors: errCount },
    errorRate,
    rss: { startBytes: result.rssStart, endBytes: result.rssEnd, growthMb: rssGrowthMb },
    histogram: histogram(Array.from(result.latencies)),
    passes,
    verdict: allPass ? 'PASS' : 'FAIL',
  };
}

// ---------------------------------------------------------------------------
// Result envelope writer
// ---------------------------------------------------------------------------

function defaultOutPath() {
  return resolve(
    process.cwd(),
    'xe-execute-2026-05-11',
    'PHASE_10_STEP_156_BENCH_RESULT.behcs-256.json',
  );
}

function writeResult(summary, opts, outPath) {
  const envelope = {
    pid: 'PHASE-10-BENCH-PID-2026-05-11-STEP-156',
    role: 'phase-10-bench',
    anchor: 'ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11',
    ts: new Date().toISOString(),
    spec: 'kernel/docs/PHASE_8_STEP_156_10K_BENCH.md',
    config: {
      url: opts.url,
      n: opts.n,
      warmup: opts.warmup,
      concurrency: opts.concurrency,
      timeoutMs: opts.timeoutMs,
    },
    summary,
    cosign: null,   // filled at envelope emit time by cosign-chain appender
  };
  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, JSON.stringify(envelope, null, 2) + '\n');
  return outPath;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const opts = parseArgs(argv);
  console.log(`[phase-10-bench] config:`, opts);

  if (!opts.execute) {
    console.log('[phase-10-bench] DRY-RUN — no envelopes will be POSTed.');
    console.log('[phase-10-bench] plan:');
    console.log(`  - POST ${opts.n} envelopes to ${opts.url}`);
    console.log(`  - warmup ${opts.warmup} envelopes (discarded)`);
    console.log(`  - concurrency ${opts.concurrency}, timeout ${opts.timeoutMs}ms`);
    console.log(`  - 5xx-window abort: > ${ABORT_5XX_RATE * 100}% in last ${ABORT_5XX_WINDOW}`);
    console.log(`  - pass thresholds:`, PASS);
    console.log('[phase-10-bench] re-run with --execute to actually POST.');
    exit(0);
  }

  console.log('[phase-10-bench] pre-flight...');
  const pf = await preflight(opts.url, opts.timeoutMs);
  if (!pf.ok) {
    console.error('[phase-10-bench] pre-flight FAILED:', pf.error);
    exit(1);
  }
  console.log(`[phase-10-bench] pre-flight OK (origin=${pf.origin}, status=${pf.status})`);

  console.log('[phase-10-bench] starting bench...');
  const t0 = hrtime.bigint();
  const result = await runBench(opts);
  const t1 = hrtime.bigint();
  console.log(`[phase-10-bench] bench complete in ${Number(t1 - t0) / 1e9}s (aborted=${result.aborted})`);

  const summary = summarize(result);
  const outPath = opts.outPath || defaultOutPath();
  writeResult(summary, opts, outPath);

  console.log(`[phase-10-bench] verdict=${summary.verdict}`);
  console.log(`[phase-10-bench]   throughput=${summary.throughputEnvPerSec.toFixed(2)} env/s`);
  console.log(`[phase-10-bench]   p50=${summary.latency.p50.toFixed(2)}ms  p95=${summary.latency.p95.toFixed(2)}ms  p99=${summary.latency.p99.toFixed(2)}ms`);
  console.log(`[phase-10-bench]   rss-growth=${summary.rss.growthMb.toFixed(2)}MB`);
  console.log(`[phase-10-bench]   result envelope -> ${outPath}`);

  exit(summary.verdict === 'PASS' ? 0 : 1);
}

main().catch((err) => {
  console.error('[phase-10-bench] fatal:', err);
  exit(1);
});
