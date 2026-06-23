use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use asolaria_kernel_core::envelope::fedenv::{self, FedenvView};
use asolaria_kernel_core::{FEDERATION_ANCHOR_PID, KERNEL_VERSION};
use asolaria_server_agent_runtime::{spawn_child_agent_count, AgentRegistry};
use asolaria_server_cosign_ledger::CosignChain;
use asolaria_server_gnn_oracle::{GnnInference, ObservedFrame};
use asolaria_server_highway::check_transit;
use asolaria_server_tier_policy::{policy_for, AccessTier};
// #24 launch-plan composition: route a summon through C/D rooms -> runner lane -> spawn-gate ring.
// PURE/DRY: the /launch-plan.hbp route NEVER spawns a process (process_launch=0 always).
use asolaria_kernel_core::hookwall::HookTier;
use asolaria_kernel_core::spawn_gate::{seal_row, spawn_gate_verdict, SpawnGateInput};
use asolaria_kernel_core::syscall::{AccessTier as KernelAccessTier, HookwallVerdict};
use asolaria_server_agent_runtime::rooms::{
    room_folder_name, room_id_from_pid, substrate_for_stage, RoomStage, Substrate,
};
use asolaria_server_agent_runtime::runners::{runner_for_role, RunnerKind};
use asolaria_server_agent_runtime::AgentRole;

const DEFAULT_BIND: &str = "127.0.0.1:5088";
const DEFAULT_ROOM_ID: &str = "gnn-dispatch-bridge";
const DEFAULT_ROOM_STUB: &str = "rooms/task-manager/bridge/gnn-dispatch-bridge";

// Seat-source defaults · the PID Registration Office on the mounted backup volume.
// All reads are READ-ONLY; the server never writes into the office.
const DEFAULT_OFFICE_DIR: &str = "D:\\PID-Registration-Office\\registered";
const DEFAULT_FEED_DIR: &str = "D:\\PID-Registration-Office\\fabric-feed";
// Where the server records its OS pid so the watchdog can witness it (server mode only).
const PID_FILE_PATH: &str = "C:\\asolaria-behcs-256\\state\\host8-serve.pid";

// --- step A · /summon WORKLOAD route ---------------------------------------
// The 60D verb axis · same catalog node gaia-loader.mjs reads (verbs.hbp, the
// `|entry=<verb>|` rows). The verb chosen for a seat is catalog-dependent, so to
// stay byte-exact with node we must load THIS exact file the SAME way.
const DEFAULT_VERB_CATALOG: &str = "C:\\HyperBEHCS\\data\\catalogs\\verbs.hbp";
// Unique room dir per summon = fresh opencode session = $0 (proven free mechanism).
const DEFAULT_SUMMON_ROOT: &str = "D:\\bigpickle-rebuild\\gaia-summon-rooms";
// The $0 opencode model + the .cmd-safe JS bin (node <bin> run ...), matching
// gaia-loader's runFreeAgentNodeDirect. room-dispatcher is NOT modified.
const DEFAULT_SUMMON_MODEL: &str = "opencode/big-pickle";
const DEFAULT_OPENCODE_JS_BIN: &str =
    "C:\\Users\\acer\\AppData\\Local\\nvm\\v20.11.0\\node_modules\\opencode-ai\\bin\\opencode";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Room {
    id: String,
    room_stub: String,
    target_runtime: String,
    legacy_runtime: String,
    legacy_path: String,
    lane: String,
    swap_gate: String,
    host_handle8: String,
}

/// One registered federation seat (supervisor / chief / SoS / prof / formula / feed row).
/// Loaded READ-ONLY from the PID Registration Office. `handle8` is the 8-byte BEHCS
/// host id rendered as 16 lowercase hex chars (the office calls this the PID value).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seat {
    name: String,
    handle8: String,
    cube_bh: String,
    hilbert: String,
    class: String,
    layer: String,
    source: String,
}

/// The whole loaded seat address space plus per-source provenance counts.
#[derive(Clone, Debug, Default)]
struct SeatBook {
    seats: Vec<Seat>,
    registered_files: usize,
    feed_rows: usize,
    sources: usize,
    /// step A · the verb axis loaded for /summon (same catalog node gaia-loader reads).
    verbs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Config {
    bind: String,
    room_stub_path: Option<String>,
    cadence_frame_path: Option<String>,
    office_dir: String,
    feed_dir: String,
    once: bool,
    // step A · /summon WORKLOAD route
    verb_catalog: String,
    summon_root: String,
    summon_model: String,
    opencode_js_bin: String,
}

fn main() {
    let config = parse_args(env::args().skip(1));
    let room = config
        .room_stub_path
        .as_deref()
        .and_then(load_room_stub)
        .unwrap_or_else(default_room);

    // Load the registered seat address space READ-ONLY before we serve anything.
    let mut book = load_seats(&config.office_dir, &config.feed_dir);
    // step A · load the verb axis (read-only) so /summon resolves byte-exact vs node.
    book.verbs = load_verb_catalog(&config.verb_catalog);

    let started = Instant::now();
    let mut gnn = GnnInference::new();
    // Long-lived agent registry — the 8-byte/PID handle table BEHIND the route layer. Owns the
    // separate per-layer DispatchCounters surfaced in HOST8LIBS. E=0: registration/counting only.
    let mut registry = AgentRegistry::new();
    if config.once {
        print!("{}", render_feed(&room, &config, &book, started, &mut gnn, &registry));
        return;
    }

    let listener = match TcpListener::bind(&config.bind) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "HOST8ERR|bind={}|error={}|json=0",
                hbp_escape(&config.bind),
                hbp_escape(error.to_string())
            );
            process::exit(2);
        }
    };

    // Server mode only: record our OS pid so special-op-jesse-watchdog-kicker.mjs can
    // witness this host8-serve process. Non-fatal on any IO error (read-only office,
    // best-effort pid file). We do NOT register with the live watchdog ourselves.
    write_pid_file();

    println!(
        "HOST8LISTEN|bind={}|id={}|room_stub={}|host_handle8={}|seats={}|json=0",
        hbp_escape(&config.bind),
        hbp_escape(&room.id),
        hbp_escape(&room.room_stub),
        hbp_escape(&room.host_handle8),
        book.seats.len()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream, &room, &config, &book, started, &mut gnn, &mut registry)
            }
            Err(error) => eprintln!("HOST8ERR|accept={}|json=0", hbp_escape(error.to_string())),
        }
    }
}

/// Write the OS pid into PID_FILE_PATH so the watchdog can witness us. Best-effort:
/// creates the parent dir if missing, logs an HBP line, never panics on failure.
fn write_pid_file() {
    use std::path::Path;
    let path = Path::new(PID_FILE_PATH);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(path, format!("{}\n", process::id())) {
        Ok(()) => println!(
            "HOST8PID|file={}|os_pid={}|json=0",
            hbp_escape(PID_FILE_PATH),
            process::id()
        ),
        Err(error) => eprintln!(
            "HOST8PID|file={}|os_pid={}|error={}|json=0",
            hbp_escape(PID_FILE_PATH),
            process::id(),
            hbp_escape(error.to_string())
        ),
    }
}

fn parse_args<I>(args: I) -> Config
where
    I: IntoIterator<Item = String>,
{
    let mut config = Config {
        bind: DEFAULT_BIND.to_string(),
        room_stub_path: None,
        cadence_frame_path: env::var("ASOLARIA_CADENCE_FRAME").ok(),
        office_dir: DEFAULT_OFFICE_DIR.to_string(),
        feed_dir: DEFAULT_FEED_DIR.to_string(),
        once: false,
        verb_catalog: env::var("HOST8_VERB_CATALOG")
            .unwrap_or_else(|_| DEFAULT_VERB_CATALOG.to_string()),
        summon_root: env::var("GAIA_SUMMON_ROOT")
            .unwrap_or_else(|_| DEFAULT_SUMMON_ROOT.to_string()),
        summon_model: env::var("SUMMON_MODEL")
            .unwrap_or_else(|_| DEFAULT_SUMMON_MODEL.to_string()),
        opencode_js_bin: env::var("OPENCODE_JS_BIN")
            .unwrap_or_else(|_| DEFAULT_OPENCODE_JS_BIN.to_string()),
    };
    let args: Vec<String> = args.into_iter().collect();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                if let Some(value) = args.get(i + 1) {
                    config.bind = value.clone();
                    i += 1;
                }
            }
            "--room-stub" => {
                if let Some(value) = args.get(i + 1) {
                    config.room_stub_path = Some(value.clone());
                    i += 1;
                }
            }
            "--office" => {
                if let Some(value) = args.get(i + 1) {
                    config.office_dir = value.clone();
                    i += 1;
                }
            }
            "--feed" => {
                if let Some(value) = args.get(i + 1) {
                    config.feed_dir = value.clone();
                    i += 1;
                }
            }
            "--verb-catalog" => {
                if let Some(value) = args.get(i + 1) {
                    config.verb_catalog = value.clone();
                    i += 1;
                }
            }
            "--summon-root" => {
                if let Some(value) = args.get(i + 1) {
                    config.summon_root = value.clone();
                    i += 1;
                }
            }
            "--cadence-frame" => {
                if let Some(value) = args.get(i + 1) {
                    config.cadence_frame_path = Some(value.clone());
                    i += 1;
                }
            }
            "--once" => config.once = true,
            _ => {}
        }
        i += 1;
    }
    config
}

fn default_room() -> Room {
    Room {
        id: DEFAULT_ROOM_ID.to_string(),
        room_stub: DEFAULT_ROOM_STUB.to_string(),
        target_runtime: "8byte-rust-host".to_string(),
        legacy_runtime: "node".to_string(),
        legacy_path: "tools/behcs/gnn-dispatch-bridge.mjs".to_string(),
        lane: "retire-or-host8".to_string(),
        swap_gate: "parity-before-retire".to_string(),
        host_handle8: host_handle8(&format!("{}|{}", DEFAULT_ROOM_ID, DEFAULT_ROOM_STUB)),
    }
}

fn load_room_stub(path: &str) -> Option<Room> {
    let body = fs::read_to_string(path).ok()?;
    let id = hbp_field(&body, "id").unwrap_or_else(|| DEFAULT_ROOM_ID.to_string());
    let room_stub = hbp_field(&body, "room_stub").unwrap_or_else(|| DEFAULT_ROOM_STUB.to_string());
    let host_handle8 = hbp_field(&body, "host_handle8")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| host_handle8(&format!("{}|{}", id, room_stub)));
    Some(Room {
        id,
        room_stub,
        target_runtime: hbp_field(&body, "target_runtime").unwrap_or_else(|| "8byte-rust-host".to_string()),
        legacy_runtime: hbp_field(&body, "legacy_runtime").unwrap_or_else(|| "node".to_string()),
        legacy_path: hbp_field(&body, "legacy_path").unwrap_or_default(),
        lane: hbp_field(&body, "lane").unwrap_or_else(|| "retire-or-host8".to_string()),
        swap_gate: hbp_field(&body, "swap_gate").unwrap_or_else(|| "parity-before-retire".to_string()),
        host_handle8,
    })
}

fn hbp_field(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        for part in line.split('|').skip(1) {
            if let Some((field_key, value)) = part.split_once('=') {
                if field_key == key {
                    return Some(value.trim().to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Seat loading · READ-ONLY from the PID Registration Office.
// ---------------------------------------------------------------------------

/// Pull a value for `key` out of one HBP row's pipe-fields. Accepts BOTH delimiter
/// shapes the office uses: `key=value` (combined FORMULA/CHIEF/SOS/PROF rows and the
/// fabric feed) and `key|value` (per-seat sup-*/agent-* files where the tag is its own
/// field). Field 0 is the row tag and is skipped; matching is case-sensitive on the
/// exact key. For `key|value` shape the value is the field immediately after the key
/// field. Returns the first match found.
fn row_value(line: &str, key: &str) -> Option<String> {
    let fields: Vec<&str> = line.split('|').collect();
    // key=value shape (skip field 0, the row tag).
    for part in fields.iter().skip(1) {
        if let Some((k, v)) = part.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().to_string());
            }
        }
    }
    // key|value shape: a bare field equal to `key`, value is the next field.
    for idx in 1..fields.len() {
        if fields[idx].trim() == key {
            if let Some(next) = fields.get(idx + 1) {
                return Some(next.trim().to_string());
            }
        }
    }
    None
}

/// Extract the 8-byte handle (16 hex chars) from a row. Handles `PID=<h>` (combined +
/// feed `pid=`) and `PID|<h>|...` (per-seat). We try, in order: `PID=`, `pid=`, then
/// the `PID|` first-sub-field. Returns lowercased handle if it looks like hex.
fn row_handle8(line: &str) -> Option<String> {
    let raw = row_value(line, "PID").or_else(|| row_value(line, "pid"))?;
    let handle = raw.split_whitespace().next().unwrap_or("").to_string();
    if !handle.is_empty() {
        Some(handle.to_lowercase())
    } else {
        None
    }
}

/// Build a seat from one combined/feed row given its row tag. Returns None if no
/// usable handle8 is present (e.g. a header or footer row).
fn seat_from_row(line: &str, source: &str) -> Option<Seat> {
    let handle8 = row_handle8(line)?;
    // The human name is field index 1 (right after the row tag) for the combined
    // shapes (FORMULA|<name>|..., CHIEF|<name>|..., SOS|<name>|..., PROF|<name>|...).
    // For the feed REG rows the name lives in `name=<n>`.
    let name = row_value(line, "name").unwrap_or_else(|| {
        line.split('|').nth(1).unwrap_or("").trim().to_string()
    });
    Some(Seat {
        name,
        handle8,
        cube_bh: row_value(line, "CUBE_BH").unwrap_or_default(),
        hilbert: row_value(line, "HILBERT")
            .or_else(|| row_value(line, "hilbert"))
            .unwrap_or_default(),
        class: row_value(line, "CLASS")
            .or_else(|| row_value(line, "class"))
            .unwrap_or_default(),
        layer: row_value(line, "LAYER")
            .or_else(|| row_value(line, "layer"))
            .unwrap_or_default(),
        source: source.to_string(),
    })
}

/// Parse a single per-seat file body (sup-*.hbp / agent-*.hbp): one seat, fields on
/// their own `TAG|value` lines.
fn seat_from_per_seat_file(body: &str, source: &str) -> Option<Seat> {
    let mut handle8 = String::new();
    let mut name = String::new();
    let mut hilbert = String::new();
    let mut layer = String::new();
    let mut class = String::new();
    let mut cube_bh = String::new();
    for line in body.lines() {
        let line = line.trim_end();
        let mut it = line.splitn(2, '|');
        let tag = it.next().unwrap_or("").trim();
        let rest = it.next().unwrap_or("");
        match tag {
            "NAME" => name = rest.trim().to_string(),
            // PID|<handle>|birth=... — handle is the first sub-field.
            "PID" => {
                handle8 = rest
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
            }
            "HILBERT" => hilbert = rest.split('|').next().unwrap_or("").trim().to_string(),
            "LAYER" => layer = rest.split('|').next().unwrap_or("").trim().to_string(),
            "CLASS" => class = rest.split('|').next().unwrap_or("").trim().to_string(),
            "CUBE_BH" => cube_bh = rest.split('|').next().unwrap_or("").trim().to_string(),
            _ => {}
        }
    }
    if handle8.is_empty() {
        return None;
    }
    Some(Seat {
        name,
        handle8,
        cube_bh,
        hilbert,
        class,
        layer,
        source: source.to_string(),
    })
}

/// True if a combined-registration row tag carries a single registered seat.
fn is_seat_row_tag(tag: &str) -> bool {
    matches!(tag, "FORMULA" | "CHIEF" | "SOS" | "PROF")
}

/// Load every seat from the office (registered/*.hbp) and the latest fabric feed.
/// Dedup by handle8 (first-seen wins; we process combined files and the richer
/// CORPUS-REGISTRATION before WAVE2 by filename sort so full rows win over -REF rows).
/// Missing paths are skipped gracefully.
fn load_seats(office_dir: &str, feed_dir: &str) -> SeatBook {
    use std::collections::HashSet;
    let mut book = SeatBook::default();
    let mut seen: HashSet<String> = HashSet::new();

    // --- Source A: registered/*.hbp ---
    if let Ok(entries) = fs::read_dir(office_dir) {
        let mut files: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension().map(|x| x == "hbp").unwrap_or(false)
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("sup-")
                            || n.starts_with("agent-")
                            || n.contains("FORMULA-CORPUS"))
                        .unwrap_or(false)
            })
            .collect();
        files.sort();
        for path in files {
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let body = match fs::read_to_string(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            book.registered_files += 1;
            if fname.contains("FORMULA-CORPUS") {
                // Combined file: many rows, one seat per FORMULA/CHIEF/SOS/PROF row.
                for line in body.lines() {
                    let tag = line.split('|').next().unwrap_or("").trim();
                    if !is_seat_row_tag(tag) {
                        continue;
                    }
                    if let Some(seat) = seat_from_row(line, &fname) {
                        if seen.insert(seat.handle8.clone()) {
                            book.seats.push(seat);
                        }
                    }
                }
            } else {
                // Per-seat file: exactly one seat.
                if let Some(seat) = seat_from_per_seat_file(&body, &fname) {
                    if seen.insert(seat.handle8.clone()) {
                        book.seats.push(seat);
                    }
                }
            }
        }
    }

    // --- Source B: fabric-feed/supervisors-fabric-feed-*.hbp (latest by name) ---
    if let Ok(entries) = fs::read_dir(feed_dir) {
        let mut feeds: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("supervisors-fabric-feed-") && n.ends_with(".hbp"))
                    .unwrap_or(false)
            })
            .collect();
        feeds.sort();
        if let Some(latest) = feeds.last() {
            let fname = latest
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if let Ok(body) = fs::read_to_string(latest) {
                for line in body.lines() {
                    let tag = line.split('|').next().unwrap_or("").trim();
                    if tag != "REG" {
                        continue; // skip FEEDHDR / FEEDFTR
                    }
                    book.feed_rows += 1;
                    if let Some(seat) = seat_from_row(line, &fname) {
                        if seen.insert(seat.handle8.clone()) {
                            book.seats.push(seat);
                        }
                    }
                }
            }
        }
    }

    book.sources = (book.registered_files > 0) as usize + (book.feed_rows > 0) as usize;
    book
}

fn canonicalize(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_control_run = false;
    for ch in input.chars() {
        if matches!(ch, '\t' | '\r' | '\n') {
            if !in_control_run {
                output.push(' ');
            }
            in_control_run = true;
        } else {
            output.push(ch);
            in_control_run = false;
        }
    }
    output
}

fn fnv1a64(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in canonicalize(input).as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn host_handle8(input: &str) -> String {
    format!("{:016x}", fnv1a64(input))
}

// ---------------------------------------------------------------------------
// step A · /summon WORKLOAD — sha256 instance_pid (PARITY with gaia-loader.mjs)
// ---------------------------------------------------------------------------

/// Full sha256 of `input` rendered as 64 lowercase hex chars. Matches node's
/// crypto.createHash('sha256').update(s).digest('hex').
fn sha256hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

/// First 16 hex chars of sha256(input). This is node gaia-loader's `sha16(s)` =
/// `sha256hex(s).slice(0, 16)`. The fabric's BEHCS-style 8-byte (16 hex) id.
fn sha16(input: &str) -> String {
    let mut s = sha256hex(input);
    s.truncate(16);
    s
}

/// Load the verb axis from `verbs.hbp` exactly the way gaia-loader.mjs does:
/// every line containing `|entry=` contributes the first `|entry=<verb>|` capture
/// (JS regex `/\|entry=([^|]+)\|/`). Order preserved (the index is hash-selected,
/// so order is load-bearing for parity). Missing file => empty vec.
fn load_verb_catalog(path: &str) -> Vec<String> {
    let body = match fs::read_to_string(path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let mut verbs = Vec::new();
    for line in body.split('\n') {
        if !line.contains("|entry=") {
            continue;
        }
        // first `|entry=...|` occurrence; capture up to the next `|`.
        if let Some(start) = line.find("|entry=") {
            let after = &line[start + "|entry=".len()..];
            if let Some(end) = after.find('|') {
                let verb = &after[..end];
                if !verb.is_empty() {
                    verbs.push(verb.to_string());
                }
            }
        }
    }
    verbs
}

/// 60D tuple verb·noun·glyph·sha — byte-identical to gaia-loader.mjs::tuple60D.
///   verb  = verb_catalog[ parseInt(sha16(handle8+"|verb")[..8], 16) % len ]  (or "report")
///   noun  = seat name
///   glyph = sha16("glyph|"+noun+"|"+cube_bh)
///   sha   = handle8
fn tuple60d(name: &str, handle8: &str, cube_bh: &str, verb_catalog: &[String]) -> (String, String, String, String) {
    let noun = name.to_string();
    let verb = if verb_catalog.is_empty() {
        "report".to_string()
    } else {
        // parseInt(sha16(handle8+"|verb").slice(0,8), 16) % len
        let sel = sha16(&format!("{}|verb", handle8));
        // first 8 hex chars -> u32 (fits, 8 hex = 32 bits). JS parseInt(.,16) on
        // an 8-hex string is an exact integer; modulo by catalog length.
        let idx_src = u64::from_str_radix(&sel[..8], 16).unwrap_or(0);
        let idx = (idx_src % verb_catalog.len() as u64) as usize;
        verb_catalog[idx].clone()
    };
    let glyph = sha16(&format!("glyph|{}|{}", noun, cube_bh));
    let sha = handle8.to_string();
    (verb, noun, glyph, sha)
}

/// Device-distinct INSTANCE pid — byte-identical to gaia-loader.mjs::resolveAgent.
///   tupleStr     = verb "." noun "." glyph "." sha
///   instance_pid = sha16( handle8 "|" device "|" ts "|" tupleStr )
fn resolve_instance_pid(
    handle8: &str,
    device: &str,
    ts: &str,
    verb: &str,
    noun: &str,
    glyph: &str,
    sha: &str,
) -> String {
    let tuple_str = format!("{}.{}.{}.{}", verb, noun, glyph, sha);
    sha16(&format!("{}|{}|{}|{}", handle8, device, ts, tuple_str))
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn hbp_escape<T: ToString>(value: T) -> String {
    value
        .to_string()
        .chars()
        .map(|ch| match ch {
            '|' | '\r' | '\n' | '\t' => '_',
            ch if ch.is_ascii_graphic() || ch == ' ' => ch,
            _ => '_',
        })
        .take(240)
        .collect()
}

fn parse_u64_field(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if let Some(hex) = trimmed.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else if trimmed.chars().all(|ch| ch.is_ascii_hexdigit())
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
    {
        u64::from_str_radix(trimmed, 16).ok()
    } else {
        trimmed.parse::<u64>().ok()
    }
}

/// Load the latest tick-stamped observed frame from a cadence HBP file (pixels-before-GPU input).
fn load_observed_frame(path: &str) -> Option<ObservedFrame> {
    let body = fs::read_to_string(path).ok()?;
    let line = body.lines().rev().find(|line| line.contains("tick="))?;
    let tick = hbp_field(line, "tick").and_then(|v| parse_u64_field(&v))?;
    let phash = hbp_field(line, "phash").and_then(|v| parse_u64_field(&v))?;
    let frame_delta = hbp_field(line, "frame_delta")
        .and_then(|v| parse_u64_field(&v))
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let entropy_q = hbp_field(line, "entropy_q")
        .and_then(|v| parse_u64_field(&v))
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let pid_fingerprint = hbp_field(line, "pid_fingerprint")
        .and_then(|v| parse_u64_field(&v))
        .or_else(|| hbp_field(line, "pid").map(|pid| fnv1a64(&pid)))
        .unwrap_or(0);
    Some(ObservedFrame { tick, phash, frame_delta, entropy_q, pid_fingerprint })
}

fn render_feed(
    room: &Room,
    config: &Config,
    book: &SeatBook,
    started: Instant,
    gnn: &mut GnnInference,
    registry: &AgentRegistry,
) -> String {
    let chain = CosignChain::new();
    let counters = registry.counters();
    // pixels-before-GPU: readiness binds a fresh cadence frame, not a loaded model.
    let gnn_frame_source = config.cadence_frame_path.as_deref().unwrap_or("-");
    let gnn_frame_ingested = config
        .cadence_frame_path
        .as_deref()
        .and_then(load_observed_frame)
        .map(|frame| gnn.observe_frame(frame))
        .unwrap_or(false);
    let gnn_ready = gnn.is_ready();
    let gnn_engine = gnn.engine_mode();
    let gnn_model_sha16 = gnn.model_sha16().unwrap_or("-").to_string();
    let gnn_frame_tick = gnn.latest_frame().map(|f| f.tick).unwrap_or(0);
    let gnn_score_q = gnn.preview_latest_score_q().unwrap_or(0);
    let public_policy = policy_for(AccessTier::Public);
    let public_to_secret = check_transit(AccessTier::Public, AccessTier::Secret)
        .map(|verdict| verdict.authorized)
        .unwrap_or(false);
    let room_stub_source = config.room_stub_path.as_deref().unwrap_or("default-embedded");

    [
        format!(
            "HOST8HDR|schema=ASOLARIA-HOST8-SERVE|version=0.1.0|runtime=rust-std|generated_unix_s={}|json=0",
            unix_seconds()
        ),
        format!(
            "HOST8KERNEL|version={}|anchor_pid={}|server_libs=5|json=0",
            hbp_escape(KERNEL_VERSION),
            hbp_escape(FEDERATION_ANCHOR_PID)
        ),
        format!(
            "HOST8ROOM|id={}|room_stub={}|host_handle8={}|target_runtime={}|lane={}|json=0",
            hbp_escape(&room.id),
            hbp_escape(&room.room_stub),
            hbp_escape(&room.host_handle8),
            hbp_escape(&room.target_runtime),
            hbp_escape(&room.lane)
        ),
        format!(
            "HOST8PROC|os_pid={}|bind={}|uptime_s={}|room_stub_source={}|json=0",
            process::id(),
            hbp_escape(&config.bind),
            started.elapsed().as_secs(),
            hbp_escape(room_stub_source)
        ),
        format!(
            "HOST8LIBS|sys_fork_spawn_count={}|virtual_registered={}|omnidispatch_routed={}|receipt_gated_helper={}|opencode_free_agent_call={}|os_process_spawn={}|ambiguous_held={}|cosign_head={}|gnn_ready={}|gnn_engine={}|gnn_model_sha16={}|gnn_frame_source={}|gnn_frame_ingested={}|gnn_frame_tick={}|gnn_score_q={}|public_policy={:?}|public_to_secret_authorized={}|json=0",
            spawn_child_agent_count(),
            counters.virtual_registered,
            counters.omnidispatch_routed,
            counters.receipt_gated_helper,
            counters.opencode_free_agent_call,
            counters.os_process_spawn,
            counters.ambiguous_held,
            hbp_escape(chain.head()),
            if gnn_ready { 1 } else { 0 },
            hbp_escape(gnn_engine),
            hbp_escape(&gnn_model_sha16),
            hbp_escape(gnn_frame_source),
            if gnn_frame_ingested { 1 } else { 0 },
            gnn_frame_tick,
            gnn_score_q,
            public_policy.authority,
            if public_to_secret { 1 } else { 0 }
        ),
        format!(
            "HOST8LEGACY|legacy_runtime={}|legacy_path={}|node_retired=0|json=0",
            hbp_escape(&room.legacy_runtime),
            hbp_escape(&room.legacy_path)
        ),
        format!(
            "HOST8GATE|swap_gate={}|parity_proven=0|spine_touched=0|dashboard_touched=0|cosign_touched=0|json=0",
            hbp_escape(&room.swap_gate)
        ),
        format!(
            "HOST8SEATBOOK|seats={}|registered_files={}|feed_rows={}|sources={}|office={}|feed={}|json=0",
            book.seats.len(),
            book.registered_files,
            book.feed_rows,
            book.sources,
            hbp_escape(&config.office_dir),
            hbp_escape(&config.feed_dir)
        ),
        "HOST8ROUTE|path=/health.hbp|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/room.hbp|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/feed.hbp|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/seats.hbp|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/seat.hbp?h=<handle8>|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/seat/<handle8>.hbp|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/count.hbp|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/summon.hbp?h=<handle8>&device=<d>&ts=<unix>&fire=<0|1>|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/v1/envelope.hbp?caller=&target=&verb=&payload=&cube=&glyph=&cosign=&ttl=&ant=&row=[&ts=]|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/launch-plan.hbp?h=<handle8>&device=<d>&ts=<unix>&role=<hermes|sub>&score=<q>&risk=<q>|method=GET|format=HBP|json=0".to_string(),
        "HOST8ROUTE|path=/summon-batch.hbp?count=<N>&role=<hermes|sub>&score=<q>&risk=<q>|method=GET|format=HBP|json=0".to_string(),
    ]
    .join("\n")
        + "\n"
}

/// Render one seat as a single HOST8SEAT HBP line. Every value passes through
/// hbp_escape; never emits `{` or `}`.
fn render_seat_line(seat: &Seat) -> String {
    format!(
        "HOST8SEAT|handle8={}|name={}|class={}|layer={}|cube_bh={}|hilbert={}|source={}|json=0",
        hbp_escape(&seat.handle8),
        hbp_escape(&seat.name),
        hbp_escape(&seat.class),
        hbp_escape(&seat.layer),
        hbp_escape(&seat.cube_bh),
        hbp_escape(&seat.hilbert),
        hbp_escape(&seat.source)
    )
}

/// `/seats.hbp` — the whole seat address space, one HOST8SEAT line per seat.
fn render_seats(book: &SeatBook) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "HOST8SEATS|count={}|sources={}|json=0\n",
        book.seats.len(),
        book.sources
    ));
    for seat in &book.seats {
        out.push_str(&render_seat_line(seat));
        out.push('\n');
    }
    out
}

/// `/seat.hbp?h=<handle8>` and `/seat/<handle8>.hbp` — one matching seat or a 404.
fn render_seat_lookup(book: &SeatBook, handle: &str) -> String {
    let needle = handle.trim().to_lowercase();
    match book.seats.iter().find(|s| s.handle8 == needle) {
        Some(seat) => format!("{}\n", render_seat_line(seat)),
        None => format!(
            "HOST8ERR|status=404|handle8={}|reason=seat_not_found|json=0\n",
            hbp_escape(&needle)
        ),
    }
}

/// `/count.hbp` — seat totals + per-source provenance.
fn render_count(book: &SeatBook) -> String {
    format!(
        "HOST8COUNT|seats={}|registered_files={}|feed_rows={}|json=0\n",
        book.seats.len(),
        book.registered_files,
        book.feed_rows
    )
}

fn render_not_found(path: &str) -> String {
    format!("HOST8ERR|status=404|path={}|reason=not_found|json=0\n", hbp_escape(path))
}

/// Fire the PROVEN $0 opencode path for ONE summon: a unique room dir = fresh
/// opencode session = $0. Replicates room-dispatcher.mjs::runFreeAgent's EXACT
/// args `run -m <model> --dir <roomDir> <question>` via the .cmd-safe launcher
/// `node <opencode-ai/bin/opencode> ...` (same as gaia-loader runFreeAgentNodeDirect:
/// modern Node refuses to spawn a .cmd without a shell, so we drive the JS bin).
/// room-dispatcher / gaia-loader are NOT modified. Returns (exit_code, response).
fn fire_opencode(config: &Config, room_dir: &str, question: &str) -> (i32, String) {
    // unique project dir => fresh session => $0
    if let Err(error) = fs::create_dir_all(room_dir) {
        return (1, format!("room_dir_create_failed:{}", error));
    }
    // EXACT opencode args, launched through the JS bin under node (no shell).
    let output = Command::new("node")
        .arg(&config.opencode_js_bin)
        .arg("run")
        .arg("-m")
        .arg(&config.summon_model)
        .arg("--dir")
        .arg(room_dir)
        .arg(question)
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .env("FORCE_COLOR", "0")
        .output();
    match output {
        Ok(out) => {
            let exit = out.status.code().unwrap_or(-1);
            let raw = String::from_utf8_lossy(&out.stdout);
            (exit, clean_opencode_output(&raw))
        }
        Err(error) => (1, format!("spawn_failed:{}", error)),
    }
}

/// Strip ANSI/TUI control codes + the build/spinner noise lines the same way
/// room-dispatcher.mjs stripAnsi + the finish() line-filter do, then cap at 1000
/// chars (matching node) so the receipt response field is comparable.
fn clean_opencode_output(raw: &str) -> String {
    // remove CSI (\x1b[...letter) and OSC (\x1b]...\x07) sequences
    let mut no_ansi = String::with_capacity(raw.len());
    let bytes: Vec<char> = raw.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '\u{1b}' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                '[' => {
                    // CSI: skip until a letter (ascii alpha) terminator
                    i += 2;
                    while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                    continue;
                }
                ']' => {
                    // OSC: skip until BEL (\x07)
                    i += 2;
                    while i < bytes.len() && bytes[i] != '\u{07}' {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                    continue;
                }
                _ => {}
            }
        }
        no_ansi.push(bytes[i]);
        i += 1;
    }
    let kept: Vec<&str> = no_ansi
        .split('\n')
        .map(|l| l.trim_end())
        .filter(|l| {
            let t = l.trim();
            if t.is_empty() {
                return false;
            }
            // drop the same noise room-dispatcher drops: build lines, prompt echoes
            if t.contains("build · ") || t.starts_with('>') {
                return false;
            }
            true
        })
        .collect();
    let mut joined = kept.join("\n").trim().to_string();
    if joined.chars().count() > 1000 {
        joined = joined.chars().take(1000).collect();
    }
    joined
}

/// `/summon.hbp?h=<handle8>&device=<d>[&ts=<unix>][&fire=1]` — resolve (and
/// optionally fire) ONE agent the Rust way. Default fire=0 (resolve-only, safe).
/// instance_pid is byte-identical to node gaia-loader::resolveAgent.
fn render_summon(book: &SeatBook, config: &Config, query: &str) -> (bool, String) {
    let handle = query_param(query, "h").unwrap_or_default().to_lowercase();
    let device = {
        let d = query_param(query, "device").unwrap_or_default();
        if d.is_empty() { "acer".to_string() } else { d }
    };
    // ts: explicit &ts= wins (lets parity be reproduced), else server unix seconds.
    let ts = query_param(query, "ts")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| unix_seconds().to_string());
    let fire = matches!(query_param(query, "fire").as_deref(), Some("1"));

    let seat = match book.seats.iter().find(|s| s.handle8 == handle) {
        Some(seat) => seat,
        None => {
            return (
                false,
                format!(
                    "HOST8ERR|status=404|handle8={}|reason=seat_not_found|json=0\n",
                    hbp_escape(&handle)
                ),
            );
        }
    };

    let (verb, noun, glyph, sha) =
        tuple60d(&seat.name, &seat.handle8, &seat.cube_bh, &book.verbs);
    let instance_pid = resolve_instance_pid(&seat.handle8, &device, &ts, &verb, &noun, &glyph, &sha);

    // fire path: unique dir keyed by instance_pid (fresh session = $0). OFF by default.
    let (fired, cost, exit, response) = if fire {
        let room_dir = format!("{}\\summon-{}", config.summon_root, instance_pid);
        // a small, real question — the proven $0 probe
        let question = format!(
            "You are Asolaria seat {} (verb={}). Reply with one short line confirming you are summoned.",
            noun, verb
        );
        let (code, resp) = fire_opencode(config, &room_dir, &question);
        (1u8, 0u64, code, resp)
    } else {
        (0u8, 0u64, 0i32, String::new())
    };

    let body = format!(
        "HOST8SUMMON|base_handle8={}|instance_pid={}|device={}|ts={}|verb={}|noun={}|glyph={}|fired={}|cost={}|exit={}|response={}|json=0\n",
        hbp_escape(&seat.handle8),
        hbp_escape(&instance_pid),
        hbp_escape(&device),
        hbp_escape(&ts),
        hbp_escape(&verb),
        hbp_escape(&noun),
        hbp_escape(&glyph),
        fired,
        cost,
        exit,
        hbp_escape(&response),
    );
    (true, body)
}

/// Extract a single query-string parameter value (no percent-decoding needed for the
/// hex handles we serve). `query` is the raw string after `?`.
fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// `/v1/envelope.hbp?caller=&target=&verb=&payload=&cube=&glyph=&cosign=&ttl=&ant=&row=[&ts=]`
/// — validate a FEDENV-v1 envelope (PURE / E=0) via the kernel `fedenv` module and classify its
/// route. ACCEPT records ONE omnidispatch route (a DESCRIPTOR count — NO dispatch, NO process
/// launch; `os_process_spawn` stays 0, gated); REJECT carries the exact `EVT-FEDENV-REJECTED-*`
/// reason. The downstream launch is never reached here. `tools/omnidispatcher` is the parity oracle.
fn render_envelope(query: &str, registry: &mut AgentRegistry) -> String {
    let caller = query_param(query, "caller").unwrap_or_default();
    let target = query_param(query, "target").unwrap_or_default();
    let verb = query_param(query, "verb").unwrap_or_default();
    let payload = query_param(query, "payload").unwrap_or_default();
    let cube = query_param(query, "cube").unwrap_or_default();
    let glyph = query_param(query, "glyph").unwrap_or_default();
    let cosign = query_param(query, "cosign").unwrap_or_default();
    let ttl = query_param(query, "ttl").unwrap_or_default();
    let ant = query_param(query, "ant").unwrap_or_default();
    let row = query_param(query, "row").unwrap_or_default();
    let back = query_param(query, "back").unwrap_or_else(|| String::from("pid:H0000"));
    let priority = query_param(query, "priority").unwrap_or_default();
    let ts = query_param(query, "ts");
    let view = FedenvView {
        caller_pid: caller.as_str(),
        target: target.as_str(),
        verb: verb.as_str(),
        payload: payload.as_str(),
        back_address: back.as_str(),
        cube_47d: cube.as_str(),
        glyph_5: glyph.as_str(),
        cosign_token: cosign.as_str(),
        ttl_seconds: ttl.as_str(),
        antecedents: ant.as_str(),
        row_hash: row.as_str(),
        ts: ts.as_deref(),
    };
    match fedenv::validate(&view) {
        Ok(()) => {
            // ACCEPT: record exactly ONE omnidispatch route — a DESCRIPTOR count, NOT a dispatch.
            // No process is launched; os_process_spawn stays 0 (the launch lane is host8-gated).
            registry.note_omnidispatch_routed();
            let route = fedenv::resolve_target(target.as_str());
            let prio = fedenv::priority_of(priority.as_str());
            format!(
                "HOST8ENVELOPE|verdict=ACCEPT|route={:?}|priority={:?}|omnidispatch_routed={}|process_launch=0|json=0",
                route,
                prio,
                registry.counters().omnidispatch_routed
            )
        }
        Err(reason) => format!(
            "HOST8ENVELOPE|verdict=REJECT|reason={}|process_launch=0|json=0",
            hbp_escape(reason.as_event_str())
        ),
    }
}

/// Spawn syscall number used for the launch-plan seal context. Cosmetic to the verdict (which keys
/// on tier + score, not the syscall no); binds to the canonical sys_spawn number in the launch wave.
const SYS_SPAWN_GATED: u8 = 9;

fn substrate_str(s: Substrate) -> &'static str {
    match s {
        Substrate::CDrive => "C",
        Substrate::DDrive => "D",
    }
}

fn runner_kind_str(k: RunnerKind) -> &'static str {
    match k {
        RunnerKind::OpenCode => "opencode",
        RunnerKind::Hermes => "hermes",
    }
}

fn verdict_str(v: HookwallVerdict) -> &'static str {
    match v {
        HookwallVerdict::Proceed => "PROCEED",
        HookwallVerdict::Hold => "HOLD",
        HookwallVerdict::Block => "BLOCK",
    }
}

/// 16-byte HVD seal row -> 32 lowercase hex chars.
fn seal_hex(row: &[u8; 16]) -> String {
    let mut out = String::with_capacity(32);
    for b in row.iter() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// `/launch-plan.hbp?h=<handle8>&device=&ts=&role=<hermes|sub>&score=<q>&risk=<q>` — #24: compose the
/// DRY launch plan for ONE summon by routing it through the three E=0 contracts in order:
///   C/D room (rooms::room_id_from_pid -> rotating C: room) -> runner lane (runners::runner_for_role)
///   -> spawn-gate ring verdict (kernel spawn_gate::spawn_gate_verdict, BLOCK>HOLD>PROCEED) -> sealed
/// HBP receipt. It NEVER fires: `process_launch=0` ALWAYS. It only reports whether a fire WOULD be
/// permitted (`fire_allowed=1` iff the gate PROCEEDs). The actual gated fire stays in the summon path.
fn render_launch_plan(book: &SeatBook, _config: &Config, gnn_score_q: u32, query: &str) -> (bool, String) {
    let handle = query_param(query, "h").unwrap_or_default().to_lowercase();
    let device = {
        let d = query_param(query, "device").unwrap_or_default();
        if d.is_empty() {
            "acer".to_string()
        } else {
            d
        }
    };
    let ts = query_param(query, "ts")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| unix_seconds().to_string());

    let seat = match book.seats.iter().find(|s| s.handle8 == handle) {
        Some(seat) => seat,
        None => {
            return (
                false,
                format!(
                    "HOST8ERR|status=404|handle8={}|reason=seat_not_found|json=0\n",
                    hbp_escape(&handle)
                ),
            );
        }
    };

    let (verb, noun, glyph, sha) = tuple60d(&seat.name, &seat.handle8, &seat.cube_bh, &book.verbs);
    let instance_pid = resolve_instance_pid(&seat.handle8, &device, &ts, &verb, &noun, &glyph, &sha);

    // 1. C/D room routing (rooms.rs): agent rooms rotate on C: (rename-before-load = $0).
    let room_id = room_id_from_pid(&instance_pid);
    let room_folder = room_folder_name(room_id);
    let substrate = substrate_for_stage(RoomStage::AgentRoom);

    // 2. Runner lane (runners.rs): &role=hermes -> Hermes lane; else the proven $0 OpenCode lane.
    let role = match query_param(query, "role").as_deref() {
        Some("hermes") => AgentRole::Hermes,
        _ => AgentRole::SubAgent,
    };
    let runner = runner_for_role(role);

    // 3. Spawn-gate ring (kernel spawn_gate): forward score defaults to the latest GNN frame score
    //    (pixels-before-GPU), reverse risk defaults to 0; both overridable via &score= / &risk=.
    let fwd = query_param(query, "score")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(gnn_score_q);
    let rev = query_param(query, "risk")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let caller_pid =
        u64::from_str_radix(&instance_pid[..instance_pid.len().min(16)], 16).unwrap_or(0);
    let gate_in = SpawnGateInput {
        slot: (room_id % 64) as u8,
        syscall_no: SYS_SPAWN_GATED,
        caller_pid,
        tier: HookTier::Micro,
        target_tier: KernelAccessTier::Public,
        event_tags: &[],
        forward_score_q: fwd,
        reverse_risk_q: rev,
    };
    let verdict = spawn_gate_verdict(&gate_in);
    let seal = seal_row(&gate_in, verdict);
    let fire_allowed = matches!(verdict, HookwallVerdict::Proceed);

    let body = format!(
        "HOST8LAUNCHPLAN|base_handle8={}|instance_pid={}|device={}|ts={}|verb={}|noun={}|room_id={}|room_folder={}|substrate={}|runner_kind={}|runner_bin_env={}|runner_model={}|gate_verdict={}|gate_fwd_q={}|gate_rev_q={}|seal_row={}|fire_allowed={}|process_launch=0|json=0\n",
        hbp_escape(&seat.handle8),
        hbp_escape(&instance_pid),
        hbp_escape(&device),
        hbp_escape(&ts),
        hbp_escape(&verb),
        hbp_escape(&noun),
        room_id,
        hbp_escape(&room_folder),
        substrate_str(substrate),
        runner_kind_str(runner.kind),
        hbp_escape(runner.bin_env),
        hbp_escape(runner.default_model),
        verdict_str(verdict),
        fwd,
        rev,
        seal_hex(&seal),
        if fire_allowed { 1 } else { 0 },
    );
    (true, body)
}

/// `/summon-batch.hbp?count=N&role=&score=&risk=` — #24 fan-out: plan a launch for the FIRST N seats
/// (capped at 1000/request), each routed C/D room -> runner -> spawn-gate, DRY. The operator's
/// "10k and 10k prisms" expressed as a batch PLAN, not a launcher: `process_launch=0` ALWAYS, zero
/// spawns. Emits a HOST8BATCH summary (per-verdict + per-substrate tallies) then one HOST8LAUNCHPLAN
/// line per seat. Reuses `render_launch_plan` per seat (its core is the single-summon contract).
fn render_summon_batch(book: &SeatBook, config: &Config, gnn_score_q: u32, query: &str) -> String {
    const MAX_BATCH: usize = 1000;
    let requested = query_param(query, "count")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);
    let role = query_param(query, "role").unwrap_or_default();
    let score = query_param(query, "score").unwrap_or_default();
    let risk = query_param(query, "risk").unwrap_or_default();
    let n = requested.min(MAX_BATCH).min(book.seats.len());

    let mut lines = String::new();
    let (mut proceed, mut hold, mut block, mut c_rooms, mut d_rooms) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for seat in book.seats.iter().take(n) {
        let sub_query = format!(
            "h={}&role={}&score={}&risk={}",
            seat.handle8, role, score, risk
        );
        let (_ok, line) = render_launch_plan(book, config, gnn_score_q, &sub_query);
        match hbp_field(&line, "gate_verdict").as_deref() {
            Some("PROCEED") => proceed += 1,
            Some("HOLD") => hold += 1,
            _ => block += 1,
        }
        match hbp_field(&line, "substrate").as_deref() {
            Some("C") => c_rooms += 1,
            Some("D") => d_rooms += 1,
            _ => {}
        }
        lines.push_str(&line);
    }

    let mut out = format!(
        "HOST8BATCH|requested={}|planned={}|seats_available={}|cap={}|proceed={}|hold={}|block={}|c_rooms={}|d_rooms={}|process_launch=0|json=0\n",
        requested,
        n,
        book.seats.len(),
        MAX_BATCH,
        proceed,
        hold,
        block,
        c_rooms,
        d_rooms
    );
    out.push_str(&lines);
    out
}

fn handle_client(
    mut stream: TcpStream,
    room: &Room,
    config: &Config,
    book: &SeatBook,
    started: Instant,
    gnn: &mut GnnInference,
    registry: &mut AgentRegistry,
) {
    let mut buffer = [0u8; 2048];
    let read = match stream.read(&mut buffer) {
        Ok(read) => read,
        Err(_) => return,
    };
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first_line = request.lines().next().unwrap_or("");
    let target = first_line.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    let (status, body) = match path {
        "/" | "/health.hbp" | "/room.hbp" | "/feed.hbp" => {
            ("200 OK", render_feed(room, config, book, started, gnn, registry))
        }
        "/seats.hbp" => ("200 OK", render_seats(book)),
        "/count.hbp" => ("200 OK", render_count(book)),
        "/summon.hbp" => {
            let (ok, body) = render_summon(book, config, query);
            (if ok { "200 OK" } else { "404 Not Found" }, body)
        }
        "/v1/envelope.hbp" => ("200 OK", render_envelope(query, registry)),
        "/launch-plan.hbp" => {
            // GNN frame score (pixels-before-GPU) is the default gate forward-score; overridable by &score=.
            let gnn_score_q = gnn.preview_latest_score_q().unwrap_or(0);
            let (ok, body) = render_launch_plan(book, config, gnn_score_q, query);
            (if ok { "200 OK" } else { "404 Not Found" }, body)
        }
        "/summon-batch.hbp" => {
            let gnn_score_q = gnn.preview_latest_score_q().unwrap_or(0);
            ("200 OK", render_summon_batch(book, config, gnn_score_q, query))
        }
        "/seat.hbp" => {
            let handle = query_param(query, "h").unwrap_or_default();
            let body = render_seat_lookup(book, &handle);
            let status = if body.starts_with("HOST8ERR|status=404") {
                "404 Not Found"
            } else {
                "200 OK"
            };
            (status, body)
        }
        p if p.starts_with("/seat/") && p.ends_with(".hbp") => {
            let handle = &p["/seat/".len()..p.len() - ".hbp".len()];
            let body = render_seat_lookup(book, handle);
            let status = if body.starts_with("HOST8ERR|status=404") {
                "404 Not Found"
            } else {
                "200 OK"
            };
            (status, body)
        }
        other => ("404 Not Found", render_not_found(other)),
    };
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalization_matches_tmhost_v1_1() {
        assert_eq!(canonicalize("a\tb\r\nc"), "a b c");
    }

    #[test]
    fn host_handle8_is_eight_bytes_hex() {
        let handle = host_handle8("gnn-dispatch-bridge|rooms/task-manager/bridge/gnn-dispatch-bridge");
        assert_eq!(handle.len(), 16);
        assert!(handle.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn parses_room_stub_fields() {
        let text = "ROOMSTUB|id=x|room_stub=rooms/x|target_runtime=8byte-rust-host|json=0\n";
        assert_eq!(hbp_field(text, "id").as_deref(), Some("x"));
        assert_eq!(hbp_field(text, "room_stub").as_deref(), Some("rooms/x"));
    }

    #[test]
    fn once_feed_is_hbp_only() {
        let config = Config {
            bind: DEFAULT_BIND.to_string(),
            room_stub_path: None,
            cadence_frame_path: None,
            office_dir: DEFAULT_OFFICE_DIR.to_string(),
            feed_dir: DEFAULT_FEED_DIR.to_string(),
            once: true,
            verb_catalog: DEFAULT_VERB_CATALOG.to_string(),
            summon_root: DEFAULT_SUMMON_ROOT.to_string(),
            summon_model: DEFAULT_SUMMON_MODEL.to_string(),
            opencode_js_bin: DEFAULT_OPENCODE_JS_BIN.to_string(),
        };
        let mut gnn = GnnInference::new();
        let feed = render_feed(&default_room(), &config, &SeatBook::default(), Instant::now(), &mut gnn, &AgentRegistry::new());
        assert!(feed.contains("HOST8HDR|"));
        assert!(feed.contains("json=0"));
        assert!(!feed.contains('{'));
        assert!(!feed.contains('}'));
    }

    #[test]
    fn parses_seat_from_combined_formula_row() {
        let line = "FORMULA|REALMATHPOS — node placement function|PID=84b4c6c420426dd7|CUBE_BH=BH.7.0.566|HILBERT=1083|GLYPH_BEHCS1024=566|GLYPH_BEHCS5=2|SECTOR=7|LANE=0|KIND=algorithm|TAG=MEASURED|CLASS=addressing-geometry (Brown-Hilbert linear index + cylinder fold + distance)|PROF=PROF-FORMULA-ADDRESSING-GEOMETRY-BROWN-HILBERT-LINEAR-INDEX-C|WHERE=x";
        let seat = seat_from_row(line, "ACER-FORMULA-CORPUS-REGISTRATION-2026-06-19.hbp")
            .expect("formula row should parse");
        assert_eq!(seat.handle8, "84b4c6c420426dd7");
        assert_eq!(seat.cube_bh, "BH.7.0.566");
        assert_eq!(seat.hilbert, "1083");
        assert!(seat.class.starts_with("addressing-geometry"));
        assert_eq!(seat.name, "REALMATHPOS — node placement function");
    }

    #[test]
    fn parses_seat_from_chief_and_sos_rows() {
        let chief = "CHIEF|FORMULA-CHIEF|PID=0155964ffc8ef1f8|HILBERT=1339|LAYER=chief|CLASS=level3_chief|GLYPH_BEHCS1024=591|CUBE_BH=BH.51.0.591|role=x|reports_to=COL-ASOLARIA-HELM";
        let s = seat_from_row(chief, "f").expect("chief parses");
        assert_eq!(s.handle8, "0155964ffc8ef1f8");
        assert_eq!(s.layer, "chief");
        assert_eq!(s.class, "level3_chief");
        assert_eq!(s.name, "FORMULA-CHIEF");
    }

    #[test]
    fn parses_seat_from_feed_reg_row() {
        let line = "REG|name=CEO-ASOLARIA-INSTANCES|pid=9198ed80b00dddee|hilbert=892|layer=helm|class=supervisor_of_supervisors_role_seat|g1024=858|g5=0|sector=?|status=CANONICAL";
        let seat = seat_from_row(line, "supervisors-fabric-feed-2026-06-10.hbp")
            .expect("feed row should parse");
        assert_eq!(seat.handle8, "9198ed80b00dddee");
        assert_eq!(seat.class, "supervisor_of_supervisors_role_seat");
        assert_eq!(seat.layer, "helm");
        assert_eq!(seat.hilbert, "892");
        assert_eq!(seat.name, "CEO-ASOLARIA-INSTANCES");
    }

    #[test]
    fn parses_seat_from_per_seat_file() {
        let body = "NAME|AGT-C4\nPID|f679158eb8ca4531|birth=2026-05-29T20:30:30.763Z|authority=X\nHILBERT|930\nLAYER|supervisor\nCLASS|hyperbehcs_supervisor_entity\n";
        let seat = seat_from_per_seat_file(body, "sup-AGT-C4-f679158eb8ca4531.hbp")
            .expect("per-seat file should parse");
        assert_eq!(seat.handle8, "f679158eb8ca4531");
        assert_eq!(seat.name, "AGT-C4");
        assert_eq!(seat.layer, "supervisor");
        assert_eq!(seat.class, "hyperbehcs_supervisor_entity");
        assert_eq!(seat.hilbert, "930");
    }

    #[test]
    fn seats_output_is_hbp_only_and_well_formed() {
        let mut book = SeatBook::default();
        book.seats.push(Seat {
            name: "REALMATHPOS — node placement".to_string(),
            handle8: "84b4c6c420426dd7".to_string(),
            cube_bh: "BH.7.0.566".to_string(),
            hilbert: "1083".to_string(),
            class: "addressing-geometry".to_string(),
            layer: "formula".to_string(),
            source: "ACER-FORMULA-CORPUS-REGISTRATION-2026-06-19.hbp".to_string(),
        });
        book.registered_files = 1;
        book.sources = 1;
        let out = render_seats(&book);
        assert!(out.contains("HOST8SEATS|"));
        assert!(out.contains("HOST8SEAT|handle8=84b4c6c420426dd7"));
        assert!(out.contains("json=0"));
        assert!(!out.contains('{'));
        assert!(!out.contains('}'));

        // count + single-seat lookup
        let count = render_count(&book);
        assert!(count.contains("HOST8COUNT|seats=1"));
        let hit = render_seat_lookup(&book, "84B4C6C420426DD7"); // case-insensitive
        assert!(hit.contains("HOST8SEAT|handle8=84b4c6c420426dd7"));
        let miss = render_seat_lookup(&book, "deadbeefdeadbeef");
        assert!(miss.contains("HOST8ERR|status=404"));
    }

    #[test]
    fn query_param_extracts_handle() {
        assert_eq!(query_param("h=abc123&x=1", "h").as_deref(), Some("abc123"));
        assert_eq!(query_param("x=1", "h"), None);
    }

    // -----------------------------------------------------------------------
    // step A · PARITY with node gaia-loader.mjs (the whole point of this route).
    // The node values below were computed by calling gaia-loader::resolveAgent
    // (and re-derived manually, both agreed) on the SAME fixed inputs:
    //   handle8 = 0155964ffc8ef1f8  (FORMULA-CHIEF)
    //   name    = FORMULA-CHIEF
    //   cube_bh = BH.51.0.591
    //   device  = acer
    //   ts      = 1750000000
    // verb catalog = C:\HyperBEHCS\data\catalogs\verbs.hbp (73 entries) -> "format"
    // Node results:
    //   glyph        = c844dff6b59b40cf
    //   instance_pid = d125579d9644c37a
    // -----------------------------------------------------------------------

    const NODE_HANDLE8: &str = "0155964ffc8ef1f8";
    const NODE_NAME: &str = "FORMULA-CHIEF";
    const NODE_CUBE_BH: &str = "BH.51.0.591";
    const NODE_DEVICE: &str = "acer";
    const NODE_TS: &str = "1750000000";
    const NODE_VERB: &str = "format";
    const NODE_GLYPH: &str = "c844dff6b59b40cf";
    const NODE_INSTANCE_PID: &str = "d125579d9644c37a";

    #[test]
    fn sha16_matches_node_crypto() {
        // node: crypto.createHash('sha256').update('abc').digest('hex').slice(0,16)
        // = ba7816bf8f01cfea (sha256("abc") = ba7816bf8f01cfea414140de5dae2223...)
        assert_eq!(sha16("abc"), "ba7816bf8f01cfea");
        assert_eq!(sha256hex("abc").len(), 64);
        assert!(sha256hex("abc").starts_with("ba7816bf8f01cfea414140de5dae2223"));
    }

    #[test]
    fn glyph_matches_node_for_formula_chief() {
        // glyph = sha16("glyph|" + noun + "|" + cube_bh)
        let glyph = sha16(&format!("glyph|{}|{}", NODE_NAME, NODE_CUBE_BH));
        assert_eq!(glyph, NODE_GLYPH);
    }

    #[test]
    fn resolve_instance_pid_matches_node_with_baked_verb() {
        // Catalog-independent: feed the verb node selected ("format") directly and
        // assert the device-distinct instance pid is byte-equal to node's.
        let pid = resolve_instance_pid(
            NODE_HANDLE8,
            NODE_DEVICE,
            NODE_TS,
            NODE_VERB,
            NODE_NAME,
            NODE_GLYPH,
            NODE_HANDLE8, // sha = handle8
        );
        assert_eq!(
            pid, NODE_INSTANCE_PID,
            "Rust instance_pid must equal node gaia-loader resolveAgent for fixed FORMULA-CHIEF inputs"
        );
    }

    #[test]
    fn full_tuple_and_pid_parity_via_real_catalog() {
        // End-to-end: load the SAME verbs.hbp node read, build the tuple, derive the
        // instance pid, and assert every component matches node. Skips gracefully if
        // the catalog file is absent on this host (the baked-verb test above still
        // proves the hash chain). When present, this proves verb-selection parity too.
        let verbs = load_verb_catalog(DEFAULT_VERB_CATALOG);
        if verbs.is_empty() {
            eprintln!(
                "PARITY-NOTE: {} absent/empty; verb-selection parity asserted by baked-verb test only",
                DEFAULT_VERB_CATALOG
            );
            return;
        }
        let (verb, noun, glyph, sha) =
            tuple60d(NODE_NAME, NODE_HANDLE8, NODE_CUBE_BH, &verbs);
        assert_eq!(verb, NODE_VERB, "verb selection must match node");
        assert_eq!(noun, NODE_NAME);
        assert_eq!(glyph, NODE_GLYPH);
        assert_eq!(sha, NODE_HANDLE8);
        let pid = resolve_instance_pid(NODE_HANDLE8, NODE_DEVICE, NODE_TS, &verb, &noun, &glyph, &sha);
        assert_eq!(pid, NODE_INSTANCE_PID, "full-chain instance_pid must match node");
    }

    #[test]
    fn summon_route_resolves_without_firing() {
        // render_summon with fire absent => fired=0, real instance_pid, no shell.
        let mut book = SeatBook::default();
        book.seats.push(Seat {
            name: NODE_NAME.to_string(),
            handle8: NODE_HANDLE8.to_string(),
            cube_bh: NODE_CUBE_BH.to_string(),
            hilbert: "1339".to_string(),
            class: "level3_chief".to_string(),
            layer: "chief".to_string(),
            source: "test".to_string(),
        });
        book.verbs = vec![NODE_VERB.to_string()]; // single-entry => verb=format deterministically
        let config = Config {
            bind: DEFAULT_BIND.to_string(),
            room_stub_path: None,
            cadence_frame_path: None,
            office_dir: DEFAULT_OFFICE_DIR.to_string(),
            feed_dir: DEFAULT_FEED_DIR.to_string(),
            once: false,
            verb_catalog: DEFAULT_VERB_CATALOG.to_string(),
            summon_root: DEFAULT_SUMMON_ROOT.to_string(),
            summon_model: DEFAULT_SUMMON_MODEL.to_string(),
            opencode_js_bin: DEFAULT_OPENCODE_JS_BIN.to_string(),
        };
        let (ok, body) = render_summon(&book, &config, "h=0155964ffc8ef1f8&device=acer&ts=1750000000");
        assert!(ok);
        assert!(body.contains(&format!("instance_pid={}", NODE_INSTANCE_PID)));
        assert!(body.contains("fired=0"));
        assert!(body.contains("base_handle8=0155964ffc8ef1f8"));
        assert!(body.contains("verb=format"));
        assert!(body.contains("json=0"));
        assert!(!body.contains('{'));
        assert!(!body.contains('}'));
        // unknown handle => 404 HOST8ERR, no fire
        let (ok2, body2) = render_summon(&book, &config, "h=deadbeefdeadbeef&device=acer");
        assert!(!ok2);
        assert!(body2.contains("HOST8ERR|status=404"));
    }

    fn launch_plan_test_fixture() -> (SeatBook, Config) {
        let mut book = SeatBook::default();
        book.seats.push(Seat {
            name: NODE_NAME.to_string(),
            handle8: NODE_HANDLE8.to_string(),
            cube_bh: NODE_CUBE_BH.to_string(),
            hilbert: "1339".to_string(),
            class: "level3_chief".to_string(),
            layer: "chief".to_string(),
            source: "test".to_string(),
        });
        book.verbs = vec![NODE_VERB.to_string()];
        let config = Config {
            bind: DEFAULT_BIND.to_string(),
            room_stub_path: None,
            cadence_frame_path: None,
            office_dir: DEFAULT_OFFICE_DIR.to_string(),
            feed_dir: DEFAULT_FEED_DIR.to_string(),
            once: false,
            verb_catalog: DEFAULT_VERB_CATALOG.to_string(),
            summon_root: DEFAULT_SUMMON_ROOT.to_string(),
            summon_model: DEFAULT_SUMMON_MODEL.to_string(),
            opencode_js_bin: DEFAULT_OPENCODE_JS_BIN.to_string(),
        };
        (book, config)
    }

    #[test]
    fn launch_plan_composes_room_runner_gate_without_firing() {
        let (book, config) = launch_plan_test_fixture();
        // No GNN score (0) => gate HOLDs (genius not cleared) => fire_allowed=0. NEVER launches.
        let (ok, body) =
            render_launch_plan(&book, &config, 0, "h=0155964ffc8ef1f8&device=acer&ts=1750000000");
        assert!(ok);
        assert!(body.contains(&format!("instance_pid={}", NODE_INSTANCE_PID)));
        assert!(body.contains("room_id="));
        assert!(body.contains("room_folder=omni-room-behcs-256-"));
        assert!(body.contains("substrate=C")); // agent rooms rotate on C:
        assert!(body.contains("runner_kind=opencode")); // sub-agent -> $0 OpenCode lane
        assert!(body.contains("runner_model=opencode/big-pickle"));
        assert!(body.contains("gate_verdict=HOLD")); // no score => held by the ring
        assert!(body.contains("fire_allowed=0"));
        assert!(body.contains("process_launch=0")); // this route NEVER fires
        assert!(body.contains("seal_row="));
        assert!(body.contains("json=0"));
        assert!(!body.contains('{'));
        assert!(!body.contains('}'));
    }

    #[test]
    fn launch_plan_gate_proceeds_on_genius_score_but_still_does_not_launch() {
        let (book, config) = launch_plan_test_fixture();
        // forward score >= 720 (genius) and reverse risk <= 280 => gate PROCEEDs.
        let (_ok, body) = render_launch_plan(
            &book,
            &config,
            0,
            "h=0155964ffc8ef1f8&device=acer&ts=1750000000&score=800&risk=10",
        );
        assert!(body.contains("gate_verdict=PROCEED"));
        assert!(body.contains("fire_allowed=1"));
        // even when a fire WOULD be allowed, this route never launches a process.
        assert!(body.contains("process_launch=0"));
    }

    #[test]
    fn launch_plan_hermes_role_selects_hermes_lane() {
        let (book, config) = launch_plan_test_fixture();
        let (_ok, body) = render_launch_plan(&book, &config, 0, "h=0155964ffc8ef1f8&role=hermes");
        assert!(body.contains("runner_kind=hermes"));
        assert!(body.contains("runner_bin_env=ASOLARIA_HERMES_BIN"));
        assert!(body.contains("runner_model=hermes-4-70b"));
    }

    #[test]
    fn launch_plan_unknown_handle_is_404() {
        let (book, config) = launch_plan_test_fixture();
        let (ok, body) = render_launch_plan(&book, &config, 800, "h=deadbeefdeadbeef&device=acer");
        assert!(!ok);
        assert!(body.contains("HOST8ERR|status=404"));
    }

    #[test]
    fn summon_batch_plans_n_dry_with_tallies() {
        let (mut book, config) = launch_plan_test_fixture();
        // add a second seat so the batch plans 2.
        book.seats.push(Seat {
            name: "AGT-C4".to_string(),
            handle8: "f679158eb8ca4531".to_string(),
            cube_bh: "BH.4.0.100".to_string(),
            hilbert: "930".to_string(),
            class: "hyperbehcs_supervisor_entity".to_string(),
            layer: "supervisor".to_string(),
            source: "test".to_string(),
        });
        // genius score => both PROCEED; both agent rooms route to C:. DRY: process_launch=0, zero spawns.
        let out = render_summon_batch(&book, &config, 0, "count=5&score=800&risk=10");
        assert!(out.contains("HOST8BATCH|requested=5|planned=2|")); // only 2 seats available
        assert!(out.contains("proceed=2"));
        assert!(out.contains("c_rooms=2"));
        assert!(out.contains("process_launch=0"));
        assert_eq!(out.matches("HOST8LAUNCHPLAN|").count(), 2); // one plan line per planned seat
        assert!(out.contains("json=0"));
        assert!(!out.contains('{'));
        assert!(!out.contains('}'));

        // no score => both HOLD (gate not genius-cleared); still zero launches.
        let held = render_summon_batch(&book, &config, 0, "count=2");
        assert!(held.contains("hold=2"));
        assert!(held.contains("proceed=0"));
        assert!(held.contains("process_launch=0"));
    }
}
