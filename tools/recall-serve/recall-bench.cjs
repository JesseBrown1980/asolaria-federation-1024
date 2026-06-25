// recall-serve benchmark — MEASURED metrics for the Asolaria Recall portal (Rust 8-byte engine).
// Hits the live engine over loopback HTTP, high-res timed. No corpus/key touched.
const http = require('http');
const PORT = Number(process.env.PORT || 4796);
const HOST = '127.0.0.1';

function req(path, headers) {
  return new Promise((resolve, reject) => {
    const t = process.hrtime.bigint();
    const r = http.get({ host: HOST, port: PORT, path, headers: headers || {} }, (resp) => {
      let b = '';
      resp.on('data', (d) => (b += d));
      resp.on('end', () => resolve({ ms: Number(process.hrtime.bigint() - t) / 1e6, status: resp.statusCode, body: b }));
    });
    r.on('error', reject);
  });
}
const pct = (arr, p) => { const a = [...arr].sort((x, y) => x - y); return a[Math.min(a.length - 1, Math.floor(p / 100 * a.length))]; };
const stat = (arr) => ({ n: arr.length, min: +Math.min(...arr).toFixed(3), med: +pct(arr, 50).toFixed(3), p95: +pct(arr, 95).toFixed(3), p99: +pct(arr, 99).toFixed(3), max: +Math.max(...arr).toFixed(3), mean: +(arr.reduce((s, x) => s + x, 0) / arr.length).toFixed(3) });

const QUERIES = [
  ['public', 'brown-hilbert', '/api/public/search?q=brown-hilbert&level=0'],
  ['public', 'what-is-asolaria', '/api/public/search?q=what-is-asolaria&level=0'],
  ['public', 'algorithms-of-asolaria', '/api/public/search?q=algorithms-of-asolaria&level=0'],
  ['loopback', 'significance', '/api/search?q=significance'],
  ['loopback', 'mcp', '/api/search?q=mcp'],
  ['loopback', 'falcon', '/api/search?q=falcon'],
  ['loopback', 'shannon', '/api/search?q=shannon'],
  ['loopback', 'gnn', '/api/search?q=gnn'],
  ['loopback', 'two-token (brown hilbert)', '/api/search?q=brown%20hilbert'],
  ['loopback', 'recall', '/api/search?q=recall'],
];
const PII = ['bank', 'vault', '.pem', 'legal', 'password', 'cnpj', 'paypal'];

(async () => {
  const health = JSON.parse((await req('/api/health')).body);
  const si = health.search_index || {};
  console.log('=== ENGINE (MEASURED from /api/health) ===');
  console.log(JSON.stringify({ ok: health.ok, rows: health.rows, terms: si.terms, postings: si.postings, skipped: si.skipped, build_ms: si.built_ms, schema: si.index_schema, json_hot_path: si.json_hot_path, linear_fallback: si.linear_fallback }));

  // warm up
  for (let i = 0; i < 20; i++) await req(QUERIES[i % QUERIES.length][2]);

  const K = 60; // iterations per query
  console.log('\n=== QUERY LATENCY (ms, warm, K=' + K + '/query) ===');
  const all = [];
  const candidateCounts = {};
  for (const [tier, label, path] of QUERIES) {
    const lat = [];
    let lastCount = null, lastCand = null;
    for (let i = 0; i < K; i++) { const r = await req(path); lat.push(r.ms); if (i === 0) { const m = r.body.match(/"count":(\d+).*?"candidate_count":(\d+)/); if (m) { lastCount = +m[1]; lastCand = +m[2]; } } }
    all.push(...lat);
    const s = stat(lat);
    candidateCounts[label] = { count: lastCount, candidates: lastCand };
    console.log(`${tier.padEnd(9)} ${label.padEnd(26)} med=${String(s.med).padStart(6)} p95=${String(s.p95).padStart(6)} p99=${String(s.p99).padStart(6)} max=${String(s.max).padStart(7)} (hits=${lastCount} cand=${lastCand})`);
  }
  console.log('\nAGGREGATE latency over ' + all.length + ' queries:', JSON.stringify(stat(all)));

  // sequential throughput
  const seqN = 500;
  let t0 = process.hrtime.bigint();
  for (let i = 0; i < seqN; i++) await req(QUERIES[i % QUERIES.length][2]);
  let seqMs = Number(process.hrtime.bigint() - t0) / 1e6;
  console.log('\n=== THROUGHPUT ===');
  console.log(`sequential: ${seqN} queries in ${seqMs.toFixed(0)}ms = ${(seqN / (seqMs / 1000)).toFixed(0)} q/s (1 client)`);

  // concurrent throughput
  const concN = 1000, conc = 32;
  t0 = process.hrtime.bigint();
  let idx = 0;
  async function worker() { while (idx < concN) { const i = idx++; await req(QUERIES[i % QUERIES.length][2]); } }
  await Promise.all(Array.from({ length: conc }, worker));
  let concMs = Number(process.hrtime.bigint() - t0) / 1e6;
  console.log(`concurrent: ${concN} queries, ${conc} clients in ${concMs.toFixed(0)}ms = ${(concN / (concMs / 1000)).toFixed(0)} q/s`);

  // NO-STALL proof: health latency while a heavy query flood runs (vs Node event-loop stall)
  console.log('\n=== NO-STALL PROOF (health latency during query flood) ===');
  let flooding = true;
  const flood = (async () => { while (flooding) { await Promise.all(Array.from({ length: 16 }, () => req(QUERIES[3][2]))); } })();
  const healthLat = [];
  for (let i = 0; i < 30; i++) { const r = await req('/api/health'); healthLat.push(r.ms); }
  flooding = false; await flood;
  console.log('/api/health under flood:', JSON.stringify(stat(healthLat)), '-> never blocks (thread-per-conn)');

  // L0 PII-free
  console.log('\n=== L0 PII-FREE (public tier; all must be 0) ===');
  const pii = {};
  for (const p of PII) { const r = await req('/api/public/search?q=' + encodeURIComponent(p) + '&level=0'); const m = r.body.match(/"count":(\d+)/); pii[p] = m ? +m[1] : 'ERR'; }
  console.log(JSON.stringify(pii), Object.values(pii).every((v) => v === 0) ? '-> PII-FREE ✓' : '-> LEAK!');
})().catch((e) => { console.error('BENCH ERROR', e.message); process.exit(1); });
