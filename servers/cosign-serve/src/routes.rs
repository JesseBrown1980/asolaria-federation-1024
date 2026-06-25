use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use asolaria_server_cosign_ledger::py_parity::{canonical_bytes, parse_line, sha16, LAW_ANCHOR};

use crate::http::Request;
use crate::{json_pair_num, json_pair_str, now_iso, store, Shared};

pub fn route(shared: &Arc<Shared>, req: Request) -> (u16, String) {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/health") => health(shared),
        ("GET", "/api/cosign/head") => head(shared),
        ("GET", "/api/cosign/verify") => verify(shared, &req.query),
        ("GET", "/api/cosign/tail") => tail(shared, &req.query),
        ("POST", "/api/cosign/append") => append(shared, &req.body),
        _ => (
            404,
            format!(
                "{{\"ok\":false,\"error\":\"unknown route\",{}}}",
                json_pair_str("path", &req.path)
            ),
        ),
    }
}

fn health(shared: &Arc<Shared>) -> (u16, String) {
    let (seq, hash) = match shared.writer.lock() {
        Ok(w) => (w.head.head_seq, w.head.head_hash.clone()),
        Err(_) => (0, String::from("poisoned")),
    };
    (
        200,
        format!(
            "{{\"ok\":true,\"service\":\"cosign-serve\",\"port\":5091,{},{},{},\"append_shadow\":{},\"live_write\":false}}",
            json_pair_num("head_seq", seq),
            json_pair_str("head_row_hash", &hash),
            json_pair_str("law_anchor", LAW_ANCHOR),
            if shared.append_enabled { "true" } else { "false" }
        ),
    )
}

fn head(shared: &Arc<Shared>) -> (u16, String) {
    let state = match crate::resume::resume_from(&shared.live_path) {
        Ok(s) => s,
        Err(_) => match shared.writer.lock() {
            Ok(w) => w.head.clone(),
            Err(_) => Default::default(),
        },
    };
    (
        200,
        format!(
            "{{\"ok\":true,{},{}}}",
            json_pair_num("seq", state.head_seq),
            json_pair_str("row_hash", &state.head_hash)
        ),
    )
}

fn append(shared: &Arc<Shared>, body: &[u8]) -> (u16, String) {
    if !shared.append_enabled {
        return (
            403,
            String::from("{\"ok\":false,\"error\":\"append-disabled-shadow-only\"}"),
        );
    }
    let payload = match store::parse_payload(body) {
        Ok(p) => p,
        Err(_) => return (400, String::from("{\"ok\":false,\"error\":\"parse\"}")),
    };
    let mut writer = match shared.writer.lock() {
        Ok(w) => w,
        Err(_) => {
            return (
                500,
                String::from("{\"ok\":false,\"error\":\"writer-poisoned\"}"),
            )
        }
    };
    match store::append_shadow(&mut writer, payload, &now_iso()) {
        Ok(out) => (
            200,
            format!(
                "{{\"ok\":true,{},{},{}}}",
                json_pair_num("seq", out.seq),
                json_pair_str("row_hash", &out.row_hash),
                json_pair_str("antecedent_prev", &out.antecedent_prev)
            ),
        ),
        Err(_) => (
            500,
            String::from("{\"ok\":false,\"error\":\"append-failed\"}"),
        ),
    }
}

fn verify(shared: &Arc<Shared>, query: &str) -> (u16, String) {
    let use_shadow = query_param(query, "source").as_deref() == Some("shadow");
    let path = if use_shadow {
        match shared.writer.lock() {
            Ok(w) => w.shadow_path.clone(),
            Err(_) => {
                return (
                    500,
                    String::from("{\"ok\":false,\"error\":\"writer-poisoned\"}"),
                )
            }
        }
    } else {
        shared.live_path.clone()
    };
    let Ok(file) = File::open(path) else {
        return (
            200,
            String::from("{\"ok\":true,\"checked\":0,\"matched\":0,\"mismatches\":0,\"legacy\":0}"),
        );
    };
    let mut checked = 0u64;
    let mut matched = 0u64;
    let mut mismatches = 0u64;
    let mut legacy = 0u64;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(row) = parse_line(line.as_bytes()) else {
            legacy += 1;
            continue;
        };
        let Some(disk) = row.row_hash() else {
            legacy += 1;
            continue;
        };
        checked += 1;
        if sha16(&canonical_bytes(&row)) == disk {
            matched += 1;
        } else {
            mismatches += 1;
        }
    }
    (
        200,
        format!(
            "{{\"ok\":true,{},{},{},{}}}",
            json_pair_num("checked", checked),
            json_pair_num("matched", matched),
            json_pair_num("mismatches", mismatches),
            json_pair_num("legacy", legacy)
        ),
    )
}

fn tail(shared: &Arc<Shared>, query: &str) -> (u16, String) {
    let n = query_param(query, "n")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(5)
        .min(50);
    let use_shadow = query_param(query, "source").as_deref() == Some("shadow");
    let path = if use_shadow {
        match shared.writer.lock() {
            Ok(w) => w.shadow_path.clone(),
            Err(_) => {
                return (
                    500,
                    String::from("{\"ok\":false,\"error\":\"writer-poisoned\"}"),
                )
            }
        }
    } else {
        shared.live_path.clone()
    };
    let Ok(file) = File::open(path) else {
        return (200, String::from("{\"ok\":true,\"count\":0,\"rows\":[]}"));
    };
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(line);
        if rows.len() > n {
            rows.remove(0);
        }
    }
    let encoded = rows
        .iter()
        .map(|r| crate::json_escape(r))
        .collect::<Vec<_>>()
        .join(",");
    (
        200,
        format!(
            "{{\"ok\":true,\"count\":{},\"rows\":[{}]}}",
            rows.len(),
            encoded
        ),
    )
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        if k == key {
            Some(v.to_string())
        } else {
            None
        }
    })
}
