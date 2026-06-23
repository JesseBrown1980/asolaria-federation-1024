//! recall-serve — Rust HBI-backed recall search with an in-memory inverted index.
//!
//! `serve-recall.cjs` (Node, linear) event-loop-stalls on the 591k-row / 159 MB
//! corpus: one `fs.readSync`-heavy query blocks the single event loop so EVERY
//! route — `/api/health` included — times out. This serves the SAME `.hbp/.hbi`
//! corpus with the HILBRA-IDX-BEHCS-TUPLE-TEXT-V1 inverted index (term→postings,
//! pid/bh exact maps, precomputed levels) on a thread-per-connection server:
//! query = token lookup → small candidate set → O(1) seek → level-filter, in
//! tens of ms, and a slow query never blocks another.
//!
//! Parity-faithful to serve-recall.cjs / serve-recall-indexed.cjs (tokenizer,
//! level classification, HMAC, response shape). Fragment lists extracted from the
//! `.cjs` (drift-proof). Corpus is read-only and NEVER published; the key is read
//! from a file and never logged. No fire, no mint, no write.

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{env, thread};

const LEVEL_PUBLIC: i64 = 0;
const LEVEL_FEDERATION: i64 = 5;
const LEVEL_OWNER_PRIVATE: i64 = 9;
const MAX_SKEW_S: i64 = 120;
const TERM_MIN: usize = 2;
const TERM_MAX: usize = 64;
const MAX_TERMS_PER_ROW: usize = 96;
const MAX_REFS_PER_TERM: usize = 20000;

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
    level: i64,
}

struct Index {
    entries: Vec<Entry>,
    term_map: HashMap<String, Vec<u32>>,
    by_pid: HashMap<String, u32>,
    by_bh: HashMap<String, u32>,
    postings: u64,
    build_ms: u128,
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
}

fn env_or(k: &str, d: &str) -> String {
    env::var(k)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| d.to_string())
}

// ── HBI row-offset parse ─────────────────────────────────────────────────────
fn parse_idx(line: &str) -> Option<Entry> {
    let (mut pid, mut bh, mut path, mut off_s, mut len_s) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    for part in line.split('|').skip(1) {
        if let Some(i) = part.find('=') {
            match &part[..i] {
                "pid" => pid = part[i + 1..].to_string(),
                "bh" => bh = part[i + 1..].to_string(),
                "path" => path = part[i + 1..].to_string(),
                "off" => off_s = part[i + 1..].to_string(),
                "len" => len_s = part[i + 1..].to_string(),
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
        level: LEVEL_FEDERATION,
    })
}

fn seek_row_f(f: &mut File, off: u64, len: u64) -> String {
    if f.seek(SeekFrom::Start(off)).is_err() {
        return String::new();
    }
    let mut buf = vec![0u8; len as usize];
    if f.read_exact(&mut buf).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).trim_end().to_string()
}

// ── tokenizer (parity with serve-recall.cjs tokenize) ────────────────────────
fn push_tok(out: &mut Vec<String>, t: &str) {
    let t2: String = t.chars().take(TERM_MAX).collect();
    if t2.chars().count() >= TERM_MIN {
        out.push(t2);
    }
}
fn tokenize_into(value: &str, out: &mut Vec<String>) {
    let cs: Vec<char> = value.to_lowercase().chars().collect();
    let is_body = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-');
    let mut i = 0;
    while i < cs.len() {
        if cs[i].is_ascii_alphanumeric() {
            let start = i;
            i += 1;
            while i < cs.len() && (i - start) < 128 && is_body(cs[i]) {
                i += 1;
            }
            let whole: String = cs[start..i].iter().collect();
            push_tok(out, &whole);
            for part in whole.split(|c: char| !c.is_ascii_alphanumeric()) {
                if !part.is_empty() {
                    push_tok(out, part);
                }
            }
        } else {
            i += 1;
        }
    }
}
fn unique_tokens(values: &[&str], max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for v in values {
        let mut toks = Vec::new();
        tokenize_into(v, &mut toks);
        for t in toks {
            if seen.insert(t.clone()) {
                out.push(t);
                if out.len() >= max {
                    return out;
                }
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
fn assign_level(path: &str, content: &str) -> i64 {
    let p = percent_decode_lower(path).replace('\\', "/");
    if PII_PATH_FRAGMENTS.iter().any(|f| p.contains(f)) {
        return LEVEL_OWNER_PRIVATE;
    }
    let c = content.to_lowercase();
    if PII_CONTENT_FRAGMENTS.iter().any(|f| c.contains(f)) || has_long_digit_run(content) {
        return LEVEL_OWNER_PRIVATE;
    }
    if PUBLIC_CANON_PATH_FRAGMENTS.iter().any(|f| p.contains(f)) {
        return LEVEL_PUBLIC;
    }
    LEVEL_FEDERATION
}

// ── inverted index build ─────────────────────────────────────────────────────
fn build_index(hbi: &str, hbp: &str) -> Index {
    let t0 = Instant::now();
    let mut entries: Vec<Entry> = Vec::new();
    if let Ok(f) = File::open(hbi) {
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            if line.starts_with("IDX|") {
                if let Some(e) = parse_idx(&line) {
                    entries.push(e);
                }
            }
        }
    }
    let mut term_map: HashMap<String, Vec<u32>> = HashMap::new();
    let mut by_pid: HashMap<String, u32> = HashMap::new();
    let mut by_bh: HashMap<String, u32> = HashMap::new();
    let mut postings: u64 = 0;
    if let Ok(mut hf) = File::open(hbp) {
        for (i, e) in entries.iter_mut().enumerate() {
            let row = seek_row_f(&mut hf, e.off, e.len);
            e.level = assign_level(&e.path, &row);
            if !e.pid.is_empty() {
                by_pid.entry(e.pid.clone()).or_insert(i as u32);
            }
            if !e.bh.is_empty() {
                by_bh.entry(e.bh.to_lowercase()).or_insert(i as u32);
            }
            let toks = unique_tokens(&[&e.pid, &e.bh, &e.path, &row], MAX_TERMS_PER_ROW);
            for t in toks {
                let refs = term_map.entry(t).or_default();
                if refs.len() < MAX_REFS_PER_TERM {
                    refs.push(i as u32);
                }
                postings += 1; // count every (term,row) attempt (parity with Node's stat)
            }
        }
    }
    Index {
        entries,
        term_map,
        by_pid,
        by_bh,
        postings,
        build_ms: t0.elapsed().as_millis(),
    }
}

fn intersect(groups: &[&Vec<u32>]) -> Vec<u32> {
    if groups.is_empty() {
        return Vec::new();
    }
    let mut cur: HashSet<u32> = groups[0].iter().copied().collect();
    for g in &groups[1..] {
        let gs: HashSet<u32> = g.iter().copied().collect();
        cur.retain(|x| gs.contains(x));
        if cur.is_empty() {
            break;
        }
    }
    let mut v: Vec<u32> = cur.into_iter().collect();
    v.sort_unstable();
    v
}

fn refs_for_query(idx: &Index, q: &str) -> Vec<u32> {
    if q.is_empty() {
        return (0..idx.entries.len() as u32).collect();
    }
    if let Some(&r) = idx.by_pid.get(q) {
        return vec![r];
    }
    if let Some(&r) = idx.by_bh.get(q) {
        return vec![r];
    }
    let tokens = unique_tokens(&[q], 16);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<&Vec<u32>> = Vec::new();
    for t in &tokens {
        match idx.term_map.get(t) {
            Some(refs) if !refs.is_empty() => groups.push(refs),
            _ => return Vec::new(), // AND semantics: any missing token => no results
        }
    }
    intersect(&groups)
}

struct Hit<'a> {
    level: i64,
    e: &'a Entry,
    row: String,
}

fn search_indexed<'a>(
    idx: &'a Index,
    hbp: &str,
    raw_q: &str,
    raw_limit: &str,
    max_level: i64,
) -> (Vec<Hit<'a>>, usize) {
    let q = percent_decode_lower(raw_q.trim());
    let limit = raw_limit.parse::<usize>().unwrap_or(50).clamp(1, 250);
    let refs = refs_for_query(idx, &q);
    let candidate_count = refs.len();
    let mut hits = Vec::new();
    let mut hf = File::open(hbp).ok();
    for &r in &refs {
        if hits.len() >= limit {
            break;
        }
        let e = &idx.entries[r as usize];
        if e.level > max_level {
            continue;
        }
        let row = hf
            .as_mut()
            .map(|f| seek_row_f(f, e.off, e.len))
            .unwrap_or_default();
        hits.push(Hit {
            level: e.level,
            e,
            row,
        });
    }
    (hits, candidate_count)
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
            h.level, jesc(&h.e.pid), jesc(&h.e.bh), jesc(&h.e.path), jesc(&h.e.off_s), jesc(&h.e.len_s), jesc(&h.row)
        ));
    }
    s.push(']');
    s
}
fn search_response(
    colony: &str,
    mode: &str,
    q: &str,
    max_level: i64,
    candidate_count: usize,
    hits: &[Hit],
) -> String {
    format!(
        "{{\"colony\":\"{}\",\"access\":{{\"mode\":\"{}\",\"max_level\":{}}},\"q\":\"{}\",\"count\":{},\"max_level\":{},\"mode\":\"inverted-index\",\"index_schema\":\"HILBRA-IDX-BEHCS-TUPLE-TEXT-V1\",\"candidate_count\":{},\"matches\":{}}}",
        jesc(colony), mode, max_level, jesc(q), hits.len(), max_level, candidate_count, matches_json(hits)
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
    let mut hex = String::with_capacity(64);
    for b in outer.finalize() {
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
        || ts_raw.is_empty()
        || !ts_raw.bytes().all(|c| c.is_ascii_digit())
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
        ks.iter()
            .map(|k| self.get(k))
            .find(|v| !v.is_empty())
            .unwrap_or_default()
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
fn health_json(cfg: &Cfg, idx: &Index) -> String {
    format!(
        "{{\"ok\":true,\"schema\":\"asolaria.recall.rust.v1\",\"engine\":\"recall-serve(rust)\",\"colony\":\"{}\",\"owner_pid\":\"{}\",\"bind\":\"{}\",\"port\":{},\"rows\":{},\"search_index\":{{\"enabled\":true,\"index_schema\":\"HILBRA-IDX-BEHCS-TUPLE-TEXT-V1\",\"json_hot_path\":false,\"linear_fallback\":false,\"terms\":{},\"postings\":{},\"built_ms\":{}}},\"auth\":{{\"loopback_open\":true,\"remote_requires_hmac_sha256\":true,\"remote_requires_owner_pid\":true,\"key_configured\":{},\"max_skew_s\":{},\"canonical_message\":\"LINK|owner_pid|host|verb|nonce|ts_unix_s_be64\"}},\"access_levels\":{{\"public\":0,\"federation\":5,\"owner_private\":9,\"public_search_endpoint\":\"/api/public/search?q=...\"}},\"peers\":[{}],\"corpus\":{{\"local_only\":true,\"note\":\"engine publishable; HBP/HBI corpus must not be published\"}}}}",
        jesc(&cfg.colony), jesc(&cfg.owner_pid), jesc(&cfg.bind), cfg.port, idx.entries.len(),
        idx.term_map.len(), idx.postings, idx.build_ms,
        !cfg.key.is_empty(), MAX_SKEW_S,
        cfg.peers.iter().map(|(n, b)| format!("{{\"name\":\"{}\",\"base\":\"{}\"}}", jesc(n), jesc(b))).collect::<Vec<_>>().join(",")
    )
}
fn handle(mut s: TcpStream, cfg: Arc<Cfg>, idx: Arc<Index>) {
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
    let (path, query) = parts[1].split_once('?').unwrap_or((parts[1], ""));
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
            respond(
                &mut s,
                "200 OK",
                "application/json",
                &health_json(&cfg, &idx),
            );
            return;
        }
        "/api/public/search" => {
            let (hits, cc) = search_indexed(&idx, &cfg.hbp, &q, &limit, LEVEL_PUBLIC);
            search_response(&cfg.colony, "public", &q, LEVEL_PUBLIC, cc, &hits)
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
            let (hits, cc) = search_indexed(&idx, &cfg.hbp, &q, &limit, max_level);
            let mode = if is_loopback {
                "loopback"
            } else {
                "hmac-owner-pid"
            };
            search_response(&cfg.colony, mode, &q, max_level, cc, &hits)
        }
        "/" => {
            respond(&mut s, "200 OK", "text/plain; charset=utf-8",
                &format!("ASOLARIA-RECALL-RUST|colony={}|rows={}|terms={}|inverted=1|routes=/api/health,/api/public/search,/api/search|json=0",
                    cfg.colony, idx.entries.len(), idx.term_map.len()));
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
        let envk = env_or("ASOLARIA_RECALL_KEY", "");
        if !envk.is_empty() {
            envk
        } else {
            let home = env_or("USERPROFILE", env_or("HOME", ".").as_str());
            let kf = env_or(
                "ASOLARIA_RECALL_KEY_FILE",
                &format!("{}/.asolaria/recall.key", home),
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

    eprintln!(
        "RECALLSERVE|building inverted index from {} ...|json=0",
        hbi
    );
    let idx = Arc::new(build_index(&hbi, &hbp));
    let cfg = Arc::new(Cfg {
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
        "RECALLSERVE|ok=1|engine=rust-inverted|colony={}|bind={}:{}|rows={}|terms={}|postings={}|built_ms={}|key={}|peers={}|json=0",
        cfg.colony, cfg.bind, cfg.port, idx.entries.len(), idx.term_map.len(), idx.postings, idx.build_ms, !cfg.key.is_empty(), cfg.peers.len()
    );
    for stream in listener.incoming().flatten() {
        let (cfg, idx) = (cfg.clone(), idx.clone());
        thread::spawn(move || handle(stream, cfg, idx));
    }
}
