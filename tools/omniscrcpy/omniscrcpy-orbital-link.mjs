#!/usr/bin/env node
// omniscrcpy-orbital-link.mjs
// Registers a USB-free orbital visual/control endpoint as HBP/HBI tuple receipts.
// No secret material is read, printed, generated, or transmitted here.

import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve } from "node:path";

const SCHEMA = "ASOLARIA-OMNISCRPY-ORBITAL-LINK-V1";
const DEFAULT_OUT = resolve("tools/omniscrcpy/broadcasts/orbital");
const CATALOGS_60D = [
  "HILBRA",
  "HBI",
  "ASOLARIA_ATLAS_RECALL",
  "OMNISCRPY",
  "VIS_SUPERVISORS",
  "PID_ROSTER",
  "ORBITAL_ENDPOINT",
  "SHANNON_RECEIPT",
  "GAC",
  "AGENT_TEAMS"
];

function getArg(name, fallback = null) {
  const eq = `--${name}=`;
  const i = process.argv.findIndex(a => a === `--${name}` || a.startsWith(eq));
  if (i < 0) return fallback;
  const raw = process.argv[i];
  if (raw.startsWith(eq)) return raw.slice(eq.length);
  return process.argv[i + 1] ?? fallback;
}

function flag(name) {
  return process.argv.includes(`--${name}`);
}

function nowIso() {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

function sha256(s) {
  return createHash("sha256").update(s).digest("hex");
}

function hbpEscape(v) {
  return String(v ?? "")
    .replace(/[|\r\n\t]/g, "_")
    .replace(/\s+/g, "_")
    .slice(0, 512);
}

function pidFor(id, vantageCode, index) {
  const h = sha256(id).slice(0, 6).toUpperCase();
  return `AGENT-PID-H${h}-A${vantageCode}-N${String(index).padStart(2, "0")}`;
}

function makeReceipt(opts) {
  const ts = opts.ts || nowIso();
  const unix = Math.floor(Date.parse(ts) / 1000);
  const linkId = `${opts.from}->${opts.to}:${opts.hostPid8}:${opts.endpoint}`;
  const linkSha = sha256(linkId);
  const sha16 = linkSha.slice(0, 16);
  const visualSupervisorPid = pidFor("agent-falcon-omniscrcpy-orbital-visual-supervisor-01", "03", 7);
  const endpointPid = pidFor("agent-falcon-omnicoder-orbital-endpoint-01", "03", 8);
  const attachSupervisorPid = pidFor("agent-liris-falcon-orbital-attach-supervisor-01", "02", 19);

  const core = {
    schema: SCHEMA,
    ts,
    ts_unix_s: unix,
    evidence_class: opts.evidenceClass,
    source_vantage: "liris",
    measurement_surface: "operator_observed_acer_transcript",
    from: opts.from,
    to: opts.to,
    host_pid8: opts.hostPid8,
    endpoint: opts.endpoint,
    bind: opts.bind,
    bus: opts.bus,
    recall: opts.recall,
    tuple_dim: 60,
    levels: 16,
    catalogs_60d: CATALOGS_60D,
    usb_required: 0,
    secret_material: 0,
    pii: 0,
    key_method: "existing_backend_registry_or_owner_key_method; public handles only",
    link_sha16: sha16,
    roles: {
      visual_supervisor_pid: visualSupervisorPid,
      endpoint_pid: endpointPid,
      liris_attach_supervisor_pid: attachSupervisorPid
    }
  };

  const rowBase = [
    `schema=${SCHEMA}`,
    `ts=${ts}`,
    `evidence_class=${opts.evidenceClass}`,
    `from=${opts.from}`,
    `to=${opts.to}`,
    `host_pid8=${opts.hostPid8}`,
    `tuple_dim=60`,
    `levels=16`,
    `catalogs=${CATALOGS_60D.join(",")}`,
    `sha16=${sha16}`,
    `secret_material=0`,
    `pii=0`,
    `json=0`
  ].map(part => {
    const [k, ...rest] = part.split("=");
    return `${k}=${hbpEscape(rest.join("="))}`;
  }).join("|");

  const rows = [
    `ORBITALREG|${rowBase}|verb=omniscrcpy.orbital.register|source=liris|boundary=public_handles_only`,
    `ORBITALENDPOINT|${rowBase}|endpoint=${hbpEscape(opts.endpoint)}|bind=${hbpEscape(opts.bind)}|bus=${hbpEscape(opts.bus)}|recall=${hbpEscape(opts.recall)}|usb_required=0`,
    `HBIHOTPATH|${rowBase}|contract=LINK_owner_pid_host_verb_nonce_ts_unix_s_be64|key_off_wire=1|access_levels=L0,L5,L9|transport=trusted_lan_until_tls_or_tunnel`,
    `VISPIDSUP|${rowBase}|supervisor_pid=${visualSupervisorPid}|endpoint_pid=${endpointPid}|attach_supervisor_pid=${attachSupervisorPid}|dashboard_ingest=1|raw_hbi_projection=1`,
    `MAPADD|${rowBase}|target=asolaria-unified-fabric-map|node=falcon|node_role=orbital_mobile_endpoint|expandable_json=1|owner_private_default=1`
  ];

  return {
    ...core,
    rows,
    agent_rows: [
      {
        id: "agent-falcon-omnicoder-orbital-endpoint-01",
        pid: endpointPid,
        vantage: "falcon",
        role: "omnicoder-orbital-endpoint",
        tier: "T2",
        status: "active",
        registered_at: ts,
        registered_by: "LIRIS-CODEX-ORBITAL-REGISTRAR",
        phase_assignments: [5, 8],
        verify_envelope_pathRef: "tools/omniscrcpy/broadcasts/orbital/latest-falcon-orbital.hbp",
        evidence_class: opts.evidenceClass,
        endpoint: opts.endpoint,
        bus: opts.bus,
        recall: opts.recall,
        host_pid8: opts.hostPid8,
        tuple_dim: 60,
        catalogs_60d: CATALOGS_60D
      },
      {
        id: "agent-falcon-omniscrcpy-orbital-visual-supervisor-01",
        pid: visualSupervisorPid,
        vantage: "falcon",
        role: "OmniSCRPY-orbital-visual-supervisor",
        tier: "T2",
        status: "registered",
        registered_at: ts,
        registered_by: "LIRIS-CODEX-ORBITAL-REGISTRAR",
        phase_assignments: [5, 8],
        verify_envelope_pathRef: "tools/omniscrcpy/broadcasts/orbital/latest-falcon-orbital.hbp",
        evidence_class: "DESIGN_FROM_OPERATOR_OBSERVED_ACER",
        endpoint: opts.endpoint,
        tuple_dim: 60,
        catalogs_60d: CATALOGS_60D
      },
      {
        id: "agent-liris-falcon-orbital-attach-supervisor-01",
        pid: attachSupervisorPid,
        vantage: "liris",
        role: "orbital-attach-supervisor",
        tier: "T2",
        status: "registered",
        registered_at: ts,
        registered_by: "LIRIS-CODEX-ORBITAL-REGISTRAR",
        phase_assignments: [5, 8],
        verify_envelope_pathRef: "tools/omniscrcpy/broadcasts/orbital/latest-falcon-orbital.hbp",
        evidence_class: "DESIGN_PENDING_LIRIS_ATTACH",
        target_vantage: "falcon",
        key_material_boundary: "public_handles_only",
        tuple_dim: 60,
        catalogs_60d: CATALOGS_60D
      }
    ]
  };
}

async function maybePost(bus, receipt) {
  if (!flag("post")) return { attempted: false };
  const postJson = flag("post-json");
  const r = await fetch(bus, {
    method: "POST",
    headers: { "Content-Type": postJson ? "application/json" : "text/plain; charset=utf-8" },
    body: postJson
      ? JSON.stringify({ schema: receipt.schema, rows: receipt.rows, sha16: receipt.link_sha16 })
      : `${receipt.rows.join("\n")}\n`,
    signal: AbortSignal.timeout(3000)
  });
  return { attempted: true, mode: postJson ? "json_diagnostic" : "hbp_text", ok: r.ok, status: r.status, body: (await r.text()).slice(0, 200) };
}

if ((process.argv[1] || "").endsWith("omniscrcpy-orbital-link.mjs")) {
  const outDir = resolve(getArg("out-dir", DEFAULT_OUT));
  if (!existsSync(outDir)) mkdirSync(outDir, { recursive: true });

  const receipt = makeReceipt({
    ts: getArg("ts", nowIso()),
    evidenceClass: getArg("evidence-class", "OPERATOR_OBSERVED_ACER"),
    from: getArg("from", "falcon"),
    to: getArg("to", "acer"),
    hostPid8: getArg("host-pid8", "4c7e27b6bfb76666"),
    endpoint: getArg("endpoint", "http://192.168.1.6:8789"),
    bind: getArg("bind", "0.0.0.0:8789"),
    bus: getArg("bus", "http://192.168.1.9:4947/behcs/send"),
    recall: getArg("recall", "http://192.168.1.9:4796")
  });

  const stamp = receipt.ts.replace(/[:.]/g, "-");
  const stem = `orbital-link-${receipt.from}-to-${receipt.to}-${stamp}`;
  const hbpPath = resolve(outDir, `${stem}.hbp`);
  const latestHbp = resolve(outDir, "latest-falcon-orbital.hbp");
  const hbp = `${receipt.rows.join("\n")}\n`;
  writeFileSync(hbpPath, hbp, "utf8");
  writeFileSync(latestHbp, hbp, "utf8");
  let jsonPath = null;
  if (flag("json-out")) {
    jsonPath = resolve(outDir, `${stem}.json`);
    writeFileSync(jsonPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
  }

  const post = await maybePost(receipt.bus, receipt);
  if (flag("json-out")) {
    console.log(JSON.stringify({
      ok: true,
      schema: receipt.schema,
      hbpPath,
      jsonPath,
      latestHbp,
      sha16: receipt.link_sha16,
      rows: receipt.rows.length,
      agent_rows: receipt.agent_rows.length,
      post
    }, null, 2));
  } else {
    console.log([
      "ORBITALWRITE",
      `schema=${SCHEMA}`,
      `hbp=${hbpEscape(hbpPath)}`,
      `latest=${hbpEscape(latestHbp)}`,
      `sha16=${receipt.link_sha16}`,
      `rows=${receipt.rows.length}`,
      `agent_rows=${receipt.agent_rows.length}`,
      `post_attempted=${post.attempted ? 1 : 0}`,
      `post_mode=${hbpEscape(post.mode || "none")}`,
      "json=0"
    ].join("|"));
  }
}

export { makeReceipt, pidFor };
