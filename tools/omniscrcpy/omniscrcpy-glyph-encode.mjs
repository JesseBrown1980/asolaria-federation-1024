// omniscrcpy-glyph-encode.mjs — BEHCS-1024 glyph encoder for inter-agent envelopes
// Per operator directive 2026-05-12T~20:40Z: adopt the index language to drive the federation.
// Loads alphabet-1024.json + atlas-index.ndjson, provides cp/name lookup,
// emits glyph-form envelopes (tuple of cps) instead of verbose JSON+prose.

import { readFileSync, existsSync, appendFileSync } from "node:fs";
import { createHash } from "node:crypto";

const ALPHABET_PATH = "C:/asolaria-acer/data/behcs/codex/alphabet-1024.json";
const ATLAS_PATH = "D:/asolaria-whiteroom/behcs-1024-atlas/atlas-index.ndjson";
const EXTENSION_PATH = "D:/asolaria-whiteroom/behcs-1024-atlas/atlas-extension-acer-2026-05-12.ndjson";
const BUS = "http://127.0.0.1:4947/behcs/send";

const alphabet = JSON.parse(readFileSync(ALPHABET_PATH, "utf8"));
const GLYPHS = alphabet.glyphs;  // cp → glyph string (1 char each up to ~cp 256, then UTF for 256+)

// Parse atlas + extension to build cp-name maps
const NAME_TO_CP = new Map();
const CP_TO_NAME = new Map();
function ingestAtlasFile(path) {
  if (!existsSync(path)) return;
  const raw = readFileSync(path, "utf8").trim().split("\n").filter(Boolean);
  for (const line of raw) {
    const row = JSON.parse(line);
    NAME_TO_CP.set(row.name, row.cp);
    CP_TO_NAME.set(row.cp, row);
  }
}
ingestAtlasFile(ATLAS_PATH);
ingestAtlasFile(EXTENSION_PATH);  // anchored federation identities + verbs (operator-authorized 2026-05-12)

export function cpOf(name) {
  return NAME_TO_CP.has(name) ? NAME_TO_CP.get(name) : null;
}

export function nameOf(cp) {
  return CP_TO_NAME.get(cp)?.name || null;
}

export function glyphOf(cp) {
  return cp >= 0 && cp < GLYPHS.length ? GLYPHS[cp] : null;
}

// Encode an envelope as a tuple of cps. Returns { cps, glyphs, json_fallback }.
// Convention: [VERB_cp, FROM_cp, TO_cp, ANCHOR_cp, ...BODY_cps]
// For verbs/actors not in atlas, lazy-mint via sha256 → 10-bit fold into emergent cp range.
function laxMint(name) {
  // Fold name to cp in 271..1023 emergent range (per atlas cp 271+ = cohort-distillation-default)
  const h = createHash("sha256").update(name).digest();
  const v = h.readUInt32BE(0);
  return 271 + (v % (1024 - 271));
}

export function encodeEnvelope(env) {
  const verbCp = cpOf(env.verb) ?? laxMint(env.verb || "EVT-UNKNOWN");
  const fromCp = cpOf(env.from) ?? laxMint(env.from || "unknown");
  const anchorCp = 258;  // gaia-LAW-001-authority-kernel = canonical session anchor for now
  const tsCpPacked = Math.floor((Date.parse(env.ts || new Date().toISOString()) / 1000) % 1024);
  const cps = [verbCp, fromCp, anchorCp, tsCpPacked];
  // Append body.tags as cps (lax-minted)
  if (env.body?.tags) for (const t of env.body.tags) cps.push(cpOf(t) ?? laxMint(t));
  const glyphs = cps.map(glyphOf).join("");
  return {
    schema: "behcs-1024.glyph-tuple.v0",
    cps,
    glyphs,
    decode_atlas: ATLAS_PATH,
    decode_alphabet: ALPHABET_PATH,
    decode_hint: { verb: nameOf(verbCp) ?? `lax:${env.verb}`, from: nameOf(fromCp) ?? `lax:${env.from}`, anchor: nameOf(anchorCp), ts: env.ts },
    json_fallback_sha16: createHash("sha256").update(JSON.stringify(env)).digest("hex").slice(0, 16)
  };
}

export async function emitGlyph(env) {
  const encoded = encodeEnvelope(env);
  const r = await fetch(BUS, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(encoded),
    signal: AbortSignal.timeout(3000)
  });
  return { posted: r.ok, status: r.status, body: await r.text(), encoded };
}

// CLI demo
if ((process.argv[1] || "").endsWith("omniscrcpy-glyph-encode.mjs")) {
  const env = {
    verb: "ACER-TICK-007",
    from: "acer-Claude",
    ts: new Date().toISOString(),
    body: { tags: ["room:whiteroom", "class:HEARTBEAT", "verb:acer-tick", "glyph-form:first"] }
  };
  const encoded = encodeEnvelope(env);
  console.log("=== glyph-form envelope ===");
  console.log(JSON.stringify(encoded, null, 2));
  console.log("---");
  console.log("byte sizes — JSON-prose:", JSON.stringify(env).length, "vs glyph-tuple:", JSON.stringify(encoded).length, "vs glyphs only:", encoded.glyphs.length);
  emitGlyph(env).then(r => console.log("BUS POST:", r.status, r.body.slice(0, 200)));
}
