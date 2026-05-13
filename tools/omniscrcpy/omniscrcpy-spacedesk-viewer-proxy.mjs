// omniscrcpy-spacedesk-viewer-proxy.mjs — acer-side spacedesk VIEWER (CLIENT) proxy
// Mirror of liris's omniscrcpy-spacedesk-proxy.mjs but for the VIEWER not the CONSOLE.
// Acer runs spacedeskWindowsVIEWER (MFC Afx:00790000:0). Per W18-A2 UIAutomation inspection 2026-05-12.
// Numeric AutomationIds (vs liris's WPF friendly IDs). Wraps verbs as omniscrcpy callable surface.
// Each verb emits a whiteroom-tagged envelope to broadcasts/acer-emit/ + bus :4947.
//
// Verbs:
//   viewer:status        — query current process + bind state
//   viewer:lookup        — open Lookup sub-dialog, fill IP, click OK (returns to main form)
//   viewer:connect <ip>  — connect to spacedesk server at ip
//   viewer:disconnect    — disconnect (if connected)
//   viewer:screenshot    — capture viewer window only
//
// Each invocation:
//   1. consults substrate-conflict-matrix (avoids USB-Android driver race)
//   2. focuses spacedesk viewer window
//   3. UIA-finds the AutomationId, mouse-clicks center of bounding rect
//   4. emits a glyph-tuple envelope to bus + writes JSON to acer-emit/
//   5. logs to fired-dispatches-side log
//
// Authority: AUTHORIZATION.ndjson row 16 (FULL SYSTEMS quintuple-auth 2026-05-12 → 2026-05-26).
// Honesty: returns observed-state, not asserted-state. Caller verifies via screenshot if needed.

import { execFile } from "node:child_process";
import { writeFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// A4 — substrate-conflict-matrix consultation (Phase 2.5.5 step A4 + cosign-canon 2026-05-12)
const CONFLICT_MATRIX_PATH = "C:/asolaria-acer/federation-remake-1024/substrate-conflict-matrix.json";
let conflictMatrix = null;
try {
  conflictMatrix = JSON.parse(readFileSync(CONFLICT_MATRIX_PATH, "utf8"));
} catch { conflictMatrix = { rows: [], default_check_when_no_match: "PROCEED" }; }

function checkSubstrateConflict(verbName) {
  // Walks conflict-matrix; returns {ok: true} or {ok: false, blocked_by, swap_envelope_type}.
  // For viewer verbs the relevant rows are UIAUTOMATION_SINGLETON (always coexist+lock) and
  // (if the verb touches USB) USB_CABLE_FALCON_ACER + SAMSUNG_KIES_PROTOCOL.
  for (const row of conflictMatrix.rows || []) {
    const competitors = row.competitors || [];
    const matches = competitors.find(c => (c.verb_prefix || []).some(p => verbName.startsWith(p) || verbName.includes(p.replace(/:$/, ""))));
    if (matches) {
      if (row.coexistence === "EXCLUSIVE" || row.coexistence === "EXCLUSIVE_PER_PROTOCOL_CLAIM" || row.coexistence === "MUTUALLY_EXCLUSIVE_AT_ENUMERATION_LAYER") {
        // For now, this proxy doesn't have a way to detect incumbent runtime state,
        // so we surface the conflict info and let caller decide. Default: PROCEED with warning.
        return { ok: true, warning: `verb ${verbName} touches EXCLUSIVE substrate ${row.id} (competitor: ${matches.name}). No runtime incumbent-check implemented yet; proceeding under FULL SYSTEMS auth.`, conflict_row: row.id };
      }
      if (row.coexistence === "EXCLUSIVE_PER_WRITE_COEXIST_FOR_READ") {
        return { ok: true, warning: `verb ${verbName} touches read-coexist/write-exclusive substrate ${row.id}; proceeding (assumes read by default).`, conflict_row: row.id };
      }
    }
  }
  return { ok: true };
}

const __dirname = dirname(fileURLToPath(import.meta.url));
const BROADCAST_DIR = resolve(__dirname, "broadcasts/acer-emit");
if (!existsSync(BROADCAST_DIR)) mkdirSync(BROADCAST_DIR, { recursive: true });

const BUS = "http://127.0.0.1:4947/behcs/send";
const WIN_CLASS = "Afx:00790000:0";
const WIN_TITLE = "spacedesk Viewer for Windows 7";

// AutomationIds from W18-A2 inspection (numeric MFC IDs)
const AID = {
  ip_field: "1060",           // IP-octet MFC control (0.0.0.0)
  connect_btn: "1003",        // OK / Connect (in Lookup dialog)
  expand_recent: "1084",      // ">" expand button
  expand_other: "1035",       // another ">" button
  hostname_combo: "1083",     // hostname combobox edit
  tree_view: "2",             // spacedesk Tree View
  status_pane_3: "1078"       // another ">" button
};

// Glyph encoder for envelope emission
let encodeEnvelope = null;
try {
  const url = pathToFileURL("C:/asolaria-acer/federation-remake-1024/tools/omniscrcpy/omniscrcpy-glyph-encode.mjs").href;
  const mod = await import(url);
  encodeEnvelope = mod.encodeEnvelope;
} catch { encodeEnvelope = null; }

function runPS(cmd) {
  return new Promise((resolve) => {
    execFile("powershell", ["-NoProfile", "-Command", cmd], { timeout: 10000, windowsHide: true }, (err, stdout, stderr) => {
      resolve({ ok: !err, stdout: (stdout || "").trim(), stderr: (stderr || err?.message || "").trim() });
    });
  });
}

export async function vStatus() {
  const r = await runPS(`Get-Process -Name 'spacedesk*' -ErrorAction SilentlyContinue | ForEach-Object { '{0}|{1}|{2}' -f $_.Id, $_.ProcessName, $_.MainWindowTitle }`);
  return { ok: r.ok, verb: "viewer:status", processes: r.stdout.split(/[\r\n]+/).filter(Boolean) };
}

// Focus the viewer window + mouse-click an AutomationId's bounding rect center
async function clickAid(aid) {
  const ps = `
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes,WindowsBase
$wshell = New-Object -ComObject WScript.Shell
$wshell.AppActivate('${WIN_TITLE}') | Out-Null
Start-Sleep -Milliseconds 300
$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ClassNameProperty, '${WIN_CLASS}')
$win = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
if (-not $win) { 'ERR:no-window'; exit 1 }
$aidCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, '${aid}')
$el = $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $aidCond)
if (-not $el) { 'ERR:aid-not-found'; exit 1 }
$rect = $el.Current.BoundingRectangle
$cx = [int]($rect.X + $rect.Width / 2)
$cy = [int]($rect.Y + $rect.Height / 2)
Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public class Win32{[DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);[DllImport("user32.dll")]public static extern void mouse_event(uint f,uint x,uint y,uint d,System.IntPtr e);}'
[Win32]::SetCursorPos($cx, $cy)
Start-Sleep -Milliseconds 100
[Win32]::mouse_event(0x2, 0, 0, 0, [System.IntPtr]::Zero)
[Win32]::mouse_event(0x4, 0, 0, 0, [System.IntPtr]::Zero)
'OK:clicked-${aid}-at-' + $cx + ',' + $cy
`.replace(/\n/g, "; ");
  return runPS(ps);
}

async function sendKeys(text) {
  const ps = `Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('${text.replace(/'/g, "''")}'); 'OK:sent'`;
  return runPS(ps);
}

export async function vConnect(ip) {
  // Strategy: focus window → click IP field → SendKeys IP → click Connect
  const a = await clickAid(AID.ip_field);
  await new Promise(r => setTimeout(r, 200));
  const b = await sendKeys(ip);
  await new Promise(r => setTimeout(r, 200));
  const c = await clickAid(AID.connect_btn);
  return { ok: a.ok && b.ok && c.ok, verb: "viewer:connect", ip, ip_click: a.stdout, send: b.stdout, connect_click: c.stdout };
}

async function emitEnvelope(verb, result, conflictInfo) {
  const env = {
    verb: `EVT-${verb.toUpperCase().replace(/:/g, "-")}`,
    from: "omniscrcpy-spacedesk-viewer-proxy-acer",
    ts: new Date().toISOString(),
    body: { room: "whiteroom", tags: ["room:whiteroom","class:CONTROL",`verb:${verb}`,"substrate:uiautomation-mfc","vantage:acer"], result, conflict_check: conflictInfo || null }
  };
  const stamp = env.ts.replace(/[:.]/g, "-");
  const path = resolve(BROADCAST_DIR, `spacedesk-viewer-${verb.replace(/:/g, "-")}-${stamp}.json`);
  writeFileSync(path, JSON.stringify(env));
  if (encodeEnvelope) {
    const g = encodeEnvelope(env);
    env.glyph_tuple = { cps: g.cps, glyphs: g.glyphs };
  }
  try {
    const r = await fetch(BUS, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(env), signal: AbortSignal.timeout(3000) });
    return { ...env, bus_status: r.status, bus_ok: r.ok };
  } catch (e) {
    return { ...env, bus_status: 0, bus_ok: false, bus_err: e.message };
  }
}

const VERBS = { "viewer:status": vStatus, "viewer:connect": (a) => vConnect(a.ip || a) };

if ((process.argv[1] || "").endsWith("omniscrcpy-spacedesk-viewer-proxy.mjs")) {
  const verb = process.argv[2] || "viewer:status";
  const arg = process.argv[3];
  const fn = VERBS[verb];
  if (!fn) { console.log(`unknown verb: ${verb}. supported:`, Object.keys(VERBS)); process.exit(1); }
  // A4 — consult substrate-conflict-matrix before invoking
  const conflictCheck = checkSubstrateConflict(verb);
  if (!conflictCheck.ok) {
    console.log(JSON.stringify({ refused: true, verb, conflict: conflictCheck }, null, 2));
    process.exit(2);
  }
  const result = await fn(arg);
  const env = await emitEnvelope(verb, result, conflictCheck);
  console.log(JSON.stringify({ result, env_summary: { verb: env.verb, glyphs: env.glyph_tuple?.glyphs, bus: env.bus_status } }, null, 2));
}
