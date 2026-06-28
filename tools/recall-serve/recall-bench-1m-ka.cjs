// recall-serve 1,000,000-query KEEP-ALIVE stress — MEASURED, loopback, json=0 tuple-text endpoint.
const http = require('http');
const PORT = Number(process.env.PORT || 4796);
const N = Number(process.env.N || 1_000_000);
const C = Number(process.env.C || 64);
// keep-alive: reuse TCP connections (no per-request handshake) — the throughput lever.
const agent = new http.Agent({ keepAlive: true, maxSockets: C, maxFreeSockets: C });
const QS = ['brown-hilbert','significance','mcp','falcon','shannon','gnn','recall','asolaria','what-is-asolaria','algorithms-of-asolaria'];
// json=0 tuple-text hot path, limit small so we measure the engine, not body transfer
const PATHS = QS.map(q => '/api/public/search.hbp?q=' + encodeURIComponent(q) + '&level=0&limit=10');

const lat = new Float64Array(N);
let started = 0, done = 0, ok = 0, busy503 = 0, fail = 0, jsonSeen = 0;
const t0 = process.hrtime.bigint();

function one() {
  if (started >= N) return Promise.resolve(false);
  const i = started++;
  const path = PATHS[i % PATHS.length];
  return new Promise((resolve) => {
    const t = process.hrtime.bigint();
    const r = http.get({ host: '127.0.0.1', port: PORT, path, agent }, (resp) => {
      let firstByte = '';
      resp.on('data', (chunk) => { if (firstByte.length < 4) firstByte += chunk.toString('latin1').slice(0, 4); });
      resp.on('end', () => {
        lat[i] = Number(process.hrtime.bigint() - t) / 1e6;
        if (resp.statusCode === 200) ok++; else if (resp.statusCode === 503) busy503++; else fail++;
        if (firstByte.trimStart().startsWith('{')) jsonSeen++; // sanity: tuple-text must NOT be JSON
        done++; resolve(true);
      });
    });
    r.on('error', () => { lat[i] = Number(process.hrtime.bigint() - t) / 1e6; fail++; done++; resolve(true); });
  });
}
async function worker() { while (await one()) {} }

const iv = setInterval(() => {
  const el = Number(process.hrtime.bigint() - t0) / 1e9;
  console.log(`progress ${done}/${N} (${(done / N * 100).toFixed(1)}%) ${(done / el).toFixed(0)} q/s · ok=${ok} 503=${busy503} fail=${fail} json=${jsonSeen} · ${el.toFixed(0)}s`);
}, 15000);

(async () => {
  const h0 = await new Promise(r => http.get({host:'127.0.0.1',port:PORT,path:'/api/health.hbp',agent}, x=>{let b='';x.on('data',d=>b+=d);x.on('end',()=>r(b));}));
  const idxLine = h0.split('\n').find(l=>l.startsWith('HILBRAIDX')) || '';
  console.log(`ENGINE (json=0 tuple-text) ${idxLine.trim()} · keepalive=1 N=${N} C=${C} port=${PORT}`);
  await Promise.all(Array.from({ length: C }, worker));
  clearInterval(iv);
  const el = Number(process.hrtime.bigint() - t0) / 1e9;
  const a = Float64Array.from(lat).sort();
  const pc = (p) => a[Math.min(a.length - 1, Math.floor(p / 100 * a.length))];
  let s = 0; for (let k = 0; k < a.length; k++) s += a[k];
  console.log('=== 1,000,000-QUERY KEEP-ALIVE RESULT (MEASURED_ACER · json=0 hot path) ===');
  console.log(JSON.stringify({
    total: done, ok, busy_503: busy503, fail, json_responses: jsonSeen,
    wall_s: +el.toFixed(2), throughput_qps: +(done / el).toFixed(0), concurrency: C, keepalive: true,
    latency_ms: { min: +a[0].toFixed(3), p50: +pc(50).toFixed(3), p90: +pc(90).toFixed(3), p95: +pc(95).toFixed(3), p99: +pc(99).toFixed(3), p999: +pc(99.9).toFixed(3), max: +a[a.length-1].toFixed(3), mean: +(s/a.length).toFixed(3) }
  }, null, 2));
})().catch(e => { console.error('BENCH ERROR', e.message); process.exit(1); });
