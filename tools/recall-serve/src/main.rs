//! recall-serve — Rust HBI-backed recall search (the no-Node fix).
//!
//! `serve-recall.cjs` (Node) event-loop-stalls on the 591k-row / 159 MB corpus:
//! one `fs.readSync`-heavy query blocks the single event loop so EVERY route —
//! `/api/health` included — times out (measured on both colonies). This serves
//! the SAME `.hbp/.hbi` corpus with O(1) byte-offset seeks on a thread-per-
//! connection server, so a slow query never blocks another. Parity-faithful to
//! the Node engine's level classification + search semantics.
//!
//! Discipline: the corpus (`.hbp/.hbi`) is read-only and NEVER published; only
//! this engine is. The HMAC key is read from a file and never logged. No fire,
//! no mint, no write — read-only recall search.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, thread};

const LEVEL_PUBLIC: i64 = 0;
const LEVEL_FEDERATION: i64 = 5;
const LEVEL_OWNER_PRIVATE: i64 = 9;
const MAX_SKEW_S: i64 = 120;

// Extracted verbatim from serve-recall.cjs (drift-proof, not hand-typed).
const PII_PATH_FRAGMENTS: &[&str] = &[
    "legal",
    "evidence-package",
    "evidence",
    "google-support-refund",
    "support-refund-complaints",
    "refund-complaint",
    "refund",
    "bank",
    "invoice",
    "financial",
    "paypal",
    "zelle",
    "passport",
    "cnpj",
    "cpf",
    "whatsapp-rayssa",
    "beast-keys",
    "backup-keys",
    "decrypted-vault",
    "vault",
    "charm_",
    "private-key",
    "privatekey",
    "recall.key",
    ".pem",
    ".key",
    ".pk8",
    ".kdbx",
    ".keystore",
    ".jks",
    "id_rsa",
    "id_ed25519",
    "wallet.dat",
    "seed-phrase",
    "seed_phrase",
    "mnemonic",
    "credential",
    "secret",
    "password",
    "passwd",
    ".asolaria",
    "dcim",
    "sdcard",
    "falcon-dump",
    "phone-dump",
];
const PII_CONTENT_FRAGMENTS: &[&str] = &[
    "cnpj",
    "cpf ",
    "paypal",
    "zelle",
    "refund complaint",
    "customer care",
    "passport no",
    "invoice #",
];
const PUBLIC_CANON_PATH_FRAGMENTS: &[&str] = &[
    "asolaria-multi-cylinder",
    "scientific-voxel-atlas",
    "asolaria-real-model",
    "agentterms-os-dashboard",
    "asolaria-map-index",
    "archaeology-and-significance-canon",
    "brown-hilbert",
    "what-is-asolaria",
    "algorithms-of-asolaria",
    "session-update",
    "readme",
];

struct Entry {
    pid: String,
    bh: String,
    path: String,
    off: u64,
    len: u64,
    off_s: String,
    len_s: String,
}

struct Cfg {
    colony: String,
    owner_pid: String,
    bind: String,
    port: u16,
    hbp: String,
    key: String,
    allowed_owner_pids: Vec<String>,
    peers: Vec<(String, String)>,
    rows: usize,
}

fn env_or(k: &str, d: &str) -> String {
    env::var(k)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| d.to_string())
}

// ── HBI parse ────────────────────────────────────────────────────────────────
fn parse_idx(line: &str) -> Option<Entry> {
    let mut pid = String::new();
    let mut bh = String::new();
    let mut path = String::new();
    let mut off_s = String::new();
    let mut len_s = String::new();
    for part in line.split('|').skip(1) {
        if let Some(i) = part.find('=') {
            let (k, v) = (&part[..i], &part[i + 1..]);
            match k {
                "pid" => pid = v.to_string(),
                "bh" => bh = v.to_string(),
                "path" => path = v.to_string(),
                "off" => off_s = v.to_string(),
                "len" => len_s = v.to_string(),
                _ => {}
            }
        }
    }
    let off = off_s.parse::<u64>().ok()?;
    let len = len_s.parse::<u64>().ok()?;
    Some(Entry {
        pid,
        bh,
        path,
        off,
        len,
        off_s,
        len_s,
    })
}

fn load_index(hbi: &str) -> Vec<Entry> {
    let f = match File::open(hbi) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if line.starts_with("IDX|") {
            if let Some(e) = parse_idx(&line) {
                out.push(e);
            }
        }
    }
    out
}

// ── level classification (parity with assignLevel) ───────────────────────────
fn percent_decode_lower(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hexval(b[i + 1]), hexval(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_lowercase()
}
fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
fn has_long_digit_run(s: &str) -> bool {
    let mut run = 0;
    for c in s.bytes() {
        if c.is_ascii_digit() {
            run += 1;
            if run >= 14 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}
fn is_pii(path: &str, content: &str) -> bool {
    let p = percent_decode_lower(path).replace('\\', "/");
    if PII_PATH_FRAGMENTS.iter().any(|f| p.contains(f)) {
        return true;
    }
    let c = content.to_lowercase();
    if PII_CONTENT_FRAGMENTS.iter().any(|f| c.contains(f)) {
        return true;
    }
    has_long_digit_run(content)
}
fn is_public_canon(path: &str) -> bool {
    let p = percent_decode_lower(path).replace('\\', "/");
    PUBLIC_CANON_PATH_FRAGMENTS.iter().any(|f| p.contains(f))
}
fn assign_level(path: &str, content: &str) -> i64 {
    if is_pii(path, content) {
        LEVEL_OWNER_PRIVATE
    } else if is_public_canon(path) {
        LEVEL_PUBLIC
    } else {
        LEVEL_FEDERATION
    }
}

// ── O(seek) row fetch ────────────────────────────────────────────────────────
fn seek_row(hbp: &str, off: u64, len: u64) -> String {
    let mut f = match File::open(hbp) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    if f.seek(SeekFrom::Start(off)).is_err() {
        return String::new();
    }
    let mut buf = vec![0u8; len as usize];
    if f.read_exact(&mut buf).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).trim_end().to_string()
}

struct Hit {
    level: i64,
    pid: String,
    bh: String,
    path: String,
    off_s: String,
    len_s: String,
    row: String,
}

fn search_local(
    idx: &[Entry],
    hbp: &str,
    raw_q: &str,
    raw_limit: &str,
    max_level: i64,
) -> Vec<Hit> {
    let q = percent_decode_lower(raw_q.trim());
    let limit = raw_limit.parse::<usize>().unwrap_or(50).clamp(1, 250);
    let mut matches = Vec::new();
    for e in idx {
        if matches.len() >= limit {
            break;
        }
        // Node haystack: pid + " " + bh + " " + safeLowerPath(path); q is lowercased.
        let haystack = format!("{} {} {}", e.pid, e.bh, percent_decode_lower(&e.path));
        if !q.is_empty() && !haystack.contains(&q) {
            continue;
        }
        let row = seek_row(hbp, e.off, e.len);
        let level = assign_level(&e.path, &row);
        if level > max_level {
            continue;
        }
        matches.push(Hit {
            level,
            pid: e.pid.clone(),
            bh: e.bh.clone(),
            path: e.path.clone(),
            off_s: e.off_s.clone(),
            len_s: e.len_s.clone(),
            row,
        });
    }
    matches
}

// ── JSON (boundary only; internal is HBP/HBI tuple-text) ─────────────────────
fn jesc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn matches_json(hits: &[Hit]) -> String {
    let mut s = String::from("[");
    for (i, h) in hits.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"level\":{},\"index\":{{\"pid\":\"{}\",\"bh\":\"{}\",\"path\":\"{}\",\"off\":\"{}\",\"len\":\"{}\"}},\"row\":\"{}\"}}",
            h.level, jesc(&h.pid), jesc(&h.bh), jesc(&h.path), jesc(&h.off_s), jesc(&h.len_s), jesc(&h.row)
        ));
    }
    s.push(']');
    s
}

fn search_response(colony: &str, mode: &str, q: &str, max_level: i64, hits: &[Hit]) -> String {
    format!(
        "{{\"colony\":\"{}\",\"access\":{{\"mode\":\"{}\",\"max_level\":{}}},\"q\":\"{}\",\"count\":{},\"max_level\":{},\"index_schema\":\"HILBRA-IDX-BEHCS-TUPLE-TEXT-V1\",\"matches\":{}}}",
        jesc(colony), mode, max_level, jesc(q), hits.len(), max_level, matches_json(hits)
    )
}

// ── HMAC (parity with hmacHex) ───────────────────────────────────────────────
fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> String {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let mut h = Sha256::new();
        h.update(key);
        k[..32].copy_from_slice(&h.finalize());
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    let out = outer.finalize();
    let mut hex = String::with_capacity(64);
    for b in out {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

fn link_msg(owner: &str, host: &str, verb: &str, nonce: &str, ts: u64) -> Vec<u8> {
    let mut m = format!("LINK|{}|{}|{}|{}|", owner, host, verb, nonce).into_bytes();
    m.extend_from_slice(&ts.to_be_bytes());
    m
}

fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    for i in 0..a.len() {
        d |= a[i] ^ b[i];
    }
    d == 0
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Returns the granted max_level for a remote request, or None if denied.
/// Loopback is open (caller passes is_loopback). `expected_verb` per route.
fn verify_remote(h: &Headers, cfg: &Cfg, expected_verb: &str) -> Option<i64> {
    if cfg.key.is_empty() {
        return None;
    }
    let owner = h.get("x-asolaria-owner-pid");
    let host = h.get_any(&["x-asolaria-colony", "x-asolaria-host"]);
    let verb = h.get("x-asolaria-verb");
    let nonce = h.get("x-asolaria-nonce");
    let ts_raw = h.get("x-asolaria-ts");
    let hmac = h.get("x-asolaria-hmac");
    if owner.is_empty()
        || host.is_empty()
        || verb.is_empty()
        || nonce.is_empty()
        || !ts_raw.bytes().all(|c| c.is_ascii_digit())
        || ts_raw.is_empty()
        || hmac.len() != 64
        || !hmac.bytes().all(|c| c.is_ascii_hexdigit())
        || verb != expected_verb
    {
        return None;
    }
    let ts = ts_raw.parse::<i64>().ok()?;
    if (now_unix() - ts).abs() > MAX_SKEW_S {
        return None;
    }
    if !cfg.allowed_owner_pids.is_empty() && !cfg.allowed_owner_pids.iter().any(|o| o == &owner) {
        return None;
    }
    let expected = hmac_sha256_hex(
        cfg.key.as_bytes(),
        &link_msg(&owner, &host, &verb, &nonce, ts as u64),
    );
    if !ct_eq(&hmac, &expected) {
        return None;
    }
    // Owner-PID grant: OP-JESSE / OP-RAYSSA → owner-private (parity with grants 9).
    Some(LEVEL_OWNER_PRIVATE)
}

// ── tiny HTTP ────────────────────────────────────────────────────────────────
struct Headers(Vec<(String, String)>);
impl Headers {
    fn get(&self, k: &str) -> String {
        self.0
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    }
    fn get_any(&self, ks: &[&str]) -> String {
        for k in ks {
            let v = self.get(k);
            if !v.is_empty() {
                return v;
            }
        }
        String::new()
    }
}

fn query_param(query: &str, key: &str) -> String {
    for kv in query.split('&') {
        if let Some(i) = kv.find('=') {
            if &kv[..i] == key {
                return kv[i + 1..].to_string();
            }
        }
    }
    String::new()
}

fn respond(s: &mut TcpStream, status: &str, ctype: &str, body: &str) {
    let _ = write!(
        s,
        "HTTP/1.1 {}\r\ncontent-type: {}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{}",
        status, ctype, body.len(), body
    );
}

fn health_json(cfg: &Cfg) -> String {
    format!(
        "{{\"ok\":true,\"schema\":\"asolaria.recall.rust.v1\",\"engine\":\"recall-serve(rust)\",\"colony\":\"{}\",\"owner_pid\":\"{}\",\"bind\":\"{}\",\"port\":{},\"rows\":{},\"auth\":{{\"loopback_open\":true,\"remote_requires_hmac_sha256\":true,\"remote_requires_owner_pid\":true,\"key_configured\":{},\"max_skew_s\":{},\"canonical_message\":\"LINK|owner_pid|host|verb|nonce|ts_unix_s_be64\"}},\"access_levels\":{{\"public\":0,\"federation\":5,\"owner_private\":9,\"public_search_endpoint\":\"/api/public/search?q=...\"}},\"peers\":[{}],\"corpus\":{{\"local_only\":true,\"note\":\"engine publishable; HBP/HBI corpus must not be published\"}}}}",
        jesc(&cfg.colony), jesc(&cfg.owner_pid), jesc(&cfg.bind), cfg.port, cfg.rows,
        !cfg.key.is_empty(), MAX_SKEW_S,
        cfg.peers.iter().map(|(n, b)| format!("{{\"name\":\"{}\",\"base\":\"{}\"}}", jesc(n), jesc(b))).collect::<Vec<_>>().join(",")
    )
}

fn handle(mut s: TcpStream, cfg: Arc<Cfg>, idx: Arc<Vec<Entry>>) {
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let is_loopback = s.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
    let mut reader = BufReader::new(match s.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    let mut req_line = String::new();
    if reader.read_line(&mut req_line).is_err() {
        return;
    }
    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let target = parts[1];
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h == "\r\n" || h == "\n" || h.is_empty() {
            break;
        }
        if let Some(i) = h.find(':') {
            headers.push((h[..i].trim().to_string(), h[i + 1..].trim().to_string()));
        }
    }
    let h = Headers(headers);
    let q = query_param(query, "q");
    let limit = query_param(query, "limit");

    let body = match path {
        "/api/health" | "/health" => {
            respond(&mut s, "200 OK", "application/json", &health_json(&cfg));
            return;
        }
        "/api/public/search" => {
            let hits = search_local(&idx, &cfg.hbp, &q, &limit, LEVEL_PUBLIC);
            search_response(&cfg.colony, "public", &q, LEVEL_PUBLIC, &hits)
        }
        "/api/search" => {
            let max_level = if is_loopback {
                LEVEL_OWNER_PRIVATE
            } else {
                match verify_remote(&h, &cfg, "search") {
                    Some(l) => l,
                    None => {
                        respond(&mut s, "401 Unauthorized", "application/json",
                            "{\"ok\":false,\"error\":\"hmac-required\",\"hint\":\"public L0 open at /api/public/search\"}");
                        return;
                    }
                }
            };
            let hits = search_local(&idx, &cfg.hbp, &q, &limit, max_level);
            let mode = if is_loopback {
                "loopback"
            } else {
                "hmac-owner-pid"
            };
            search_response(&cfg.colony, mode, &q, max_level, &hits)
        }
        "/" => {
            respond(&mut s, "200 OK", "text/plain; charset=utf-8",
                &format!("ASOLARIA-RECALL-RUST|colony={}|rows={}|routes=/api/health,/api/public/search,/api/search|json=0",
                    cfg.colony, cfg.rows));
            return;
        }
        _ => {
            respond(
                &mut s,
                "404 Not Found",
                "application/json",
                "{\"ok\":false,\"error\":\"unknown route\"}",
            );
            return;
        }
    };
    respond(&mut s, "200 OK", "application/json", &body);
}

fn main() {
    let colony = env_or("ASOLARIA_RECALL_COLONY", "acer");
    let owner_pid = env_or("ASOLARIA_RECALL_OWNER_PID", "OP-JESSE-PID");
    let bind = env_or("ASOLARIA_RECALL_BIND", "127.0.0.1");
    let port = env_or("PORT", "4791").parse::<u16>().unwrap_or(4791);
    let dir = env_or("ASOLARIA_RECALL_DIR", "C:/asolaria-acer/recall-atlas/data");
    let basename = env_or("ASOLARIA_RECALL_BASENAME", "ASOLARIA-ACER-RECALL");
    let hbp = format!("{}/{}.hbp", dir, basename);
    let hbi = format!("{}/{}.hbi", dir, basename);
    let key = {
        // env override, else key file — read once, never logged.
        let envk = env_or("ASOLARIA_RECALL_KEY", "");
        if !envk.is_empty() {
            envk
        } else {
            let kf = env_or(
                "ASOLARIA_RECALL_KEY_FILE",
                &format!(
                    "{}/.asolaria/recall.key",
                    env_or("USERPROFILE", env_or("HOME", ".").as_str())
                ),
            );
            std::fs::read_to_string(&kf)
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        }
    };
    let allowed_owner_pids: Vec<String> = env_or(
        "ASOLARIA_RECALL_ALLOWED_OWNER_PIDS",
        "OP-JESSE-PID,OP-RAYSSA-PID",
    )
    .split(',')
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .collect();
    let peers: Vec<(String, String)> = env_or("ASOLARIA_RECALL_PEERS", "")
        .split(',')
        .filter_map(|p| {
            p.split_once('=').map(|(n, b)| {
                (
                    n.trim().to_string(),
                    b.trim().trim_end_matches('/').to_string(),
                )
            })
        })
        .filter(|(_, b)| b.starts_with("http"))
        .collect();

    let idx = Arc::new(load_index(&hbi));
    let cfg = Arc::new(Cfg {
        rows: idx.len(),
        colony,
        owner_pid,
        bind: bind.clone(),
        port,
        hbp,
        key,
        allowed_owner_pids,
        peers,
    });

    let listener = match TcpListener::bind((bind.as_str(), port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("RECALLSERVE|err|bind={}:{}|error={}|json=0", bind, port, e);
            std::process::exit(1);
        }
    };
    eprintln!(
        "RECALLSERVE|ok=1|engine=rust|colony={}|bind={}:{}|rows={}|key={}|peers={}|json=0",
        cfg.colony,
        cfg.bind,
        cfg.port,
        cfg.rows,
        !cfg.key.is_empty(),
        cfg.peers.len()
    );
    for stream in listener.incoming().flatten() {
        let (cfg, idx) = (cfg.clone(), idx.clone());
        thread::spawn(move || handle(stream, cfg, idx));
    }
}
