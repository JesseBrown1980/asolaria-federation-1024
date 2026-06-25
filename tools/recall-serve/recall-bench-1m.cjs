// recall-serve 1,000,000-query stress benchmark — MEASURED, loopback, high-res timed.
const http = require('http');
const PORT = Number(process.env.PORT || 4796);
const N = Number(process.env.N || 1_000_000);
const C = Number(process.env.C || 56); // concurrency, under the engine MAX_CONN=64 cap
const QS = ['brown-hilbert','significance','mcp','falcon','shannon','gnn','recall','asolaria','what-is-asolaria','algorithms-of-asolaria'];
// mix: public L0 search (the "search the Hilbra internet" public path)
const PATHS = QS.map(q => '/api/public/search?q=' + encodeURIComponent(q) + '&level=0');

const lat = new Float64Array(N);
let started = 0, done = 0, ok = 0, busy503 = 0, fail = 0;
const t0 = process.hrtime.bigint();

function one() {
  if (started >= N) return Promise.resolve(false);
  const i = started++;
  const path = PATHS[i % PATHS.length];
  return new Promise((resolve) => {
    const t = process.hrtime.bigint();
    const r = http.get({ host: '127.0.0.1', port: PORT, path }, (resp) => {
      resp.resume(); // drain body
      resp.on('end', () => {
        lat[i] = Number(process.hrtime.bigint() - t) / 1e6;
        if (resp.statusCode === 200) ok++; else if (resp.statusCode === 503) busy503++; else fail++;
        done++; resolve(true);
      });
    });
    r.on('error', () => { lat[i] = Number(process.hrtime.bigint() - t) / 1e6; fail++; done++; resolve(true); });
  });
}
async function worker() { while (await one()) {} }

const iv = setInterval(() => {
  const el = Number(process.hrtime.bigint() - t0) / 1e9;
  console.log(`progress ${done}/${N} (${(done / N * 100).toFixed(1)}%) ${(done / el).toFixed(0)} q/s · ok=${ok} 503=${busy503} fail=${fail} · ${el.toFixed(0)}s`);
}, 15000);

(async () => {
  const h0 = await new Promise(r => http.get({host:'127.0.0.1',port:PORT,path:'/api/health'}, x=>{let b='';x.on('data',d=>b+=d);x.on('end',()=>r(JSON.parse(b)));}));
  console.log(`ENGINE rows=${h0.rows} terms=${h0.search_index.terms} postings=${h0.search_index.postings} · N=${N} C=${C} port=${PORT}`);
  await Promise.all(Array.from({ length: C }, worker));
  clearInterval(iv);
  const el = Number(process.hrtime.bigint() - t0) / 1e9;
  const a = Float64Array.from(lat).sort();
  const pc = (p) => a[Math.min(a.length - 1, Math.floor(p / 100 * a.length))];
  let s = 0; for (let k = 0; k < a.length; k++) s += a[k];
  console.log('=== 1,000,000-QUERY RESULT (MEASURED_ACER) ===');
  console.log(JSON.stringify({
    total: done, ok, busy_503: busy503, fail,
    wall_s: +el.toFixed(2), throughput_qps: +(done / el).toFixed(0), concurrency: C,
    latency_ms: { min: +a[0].toFixed(3), p50: +pc(50).toFixed(3), p90: +pc(90).toFixed(3), p95: +pc(95).toFixed(3), p99: +pc(99).toFixed(3), p999: +pc(99.9).toFixed(3), max: +a[a.length-1].toFixed(3), mean: +(s/a.length).toFixed(3) }
  }, null, 2));
})().catch(e => { console.error('BENCH ERROR', e.message); process.exit(1); });
