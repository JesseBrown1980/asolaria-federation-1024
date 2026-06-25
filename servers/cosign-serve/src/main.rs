use std::env;
use std::fs::OpenOptions;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use asolaria_server_cosign_ledger::py_parity::{ResumeState, LAW_ANCHOR};

mod http;
mod resume;
mod routes;
mod store;

const DEFAULT_BIND: &str = "127.0.0.1:5091";
const DEFAULT_LIVE_PATH: &str = "C:/asolaria-acer/COSIGN_CHAIN.ndjson";
const DEFAULT_SHADOW_PATH: &str = "COSIGN_CHAIN.shadow.ndjson";
const MAX_CONN: usize = 128;

struct Shared {
    live_path: PathBuf,
    append_enabled: bool,
    writer: Mutex<store::Writer>,
}

fn main() -> std::io::Result<()> {
    let bind = env::var("ASOLARIA_COSIGN_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let live_path = PathBuf::from(
        env::var("ASOLARIA_COSIGN_LIVE").unwrap_or_else(|_| DEFAULT_LIVE_PATH.to_string()),
    );
    let shadow_path = PathBuf::from(
        env::var("ASOLARIA_COSIGN_SHADOW").unwrap_or_else(|_| DEFAULT_SHADOW_PATH.to_string()),
    );
    let append_enabled = env::var("ASOLARIA_COSIGN_APPEND_SHADOW")
        .map(|v| v == "1")
        .unwrap_or(false);

    store::ensure_clean_tail(&shadow_path)?;
    let head = resume::resume_from(&shadow_path)?;
    let shadow = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&shadow_path)?;
    let shared = Arc::new(Shared {
        live_path,
        append_enabled,
        writer: Mutex::new(store::Writer {
            shadow,
            head,
            shadow_path,
        }),
    });

    let listener = TcpListener::bind(&bind)?;
    println!(
        "COSIGNSERVE|bind={}|append_shadow={}|live_write=false|law_anchor={}|json=0",
        hbp_escape(&bind),
        if append_enabled { 1 } else { 0 },
        hbp_escape(LAW_ANCHOR)
    );

    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming().flatten() {
        if active.load(std::sync::atomic::Ordering::SeqCst) >= MAX_CONN {
            let _ = http::write_status(stream, 503, r#"{"ok":false,"error":"busy"}"#);
            continue;
        }
        active.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let shared = Arc::clone(&shared);
        let active2 = Arc::clone(&active);
        thread::spawn(move || {
            http::handle_conn(stream, shared);
            active2.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });
    }
    Ok(())
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = unix_utc_parts(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z")
}

fn unix_utc_parts(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let sod = secs % 86_400;
    let hour = (sod / 3_600) as u32;
    let minute = ((sod % 3_600) / 60) as u32;
    let second = (sod % 60) as u32;
    let (year, month, day) = civil_from_days(days);
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn hbp_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '|' | '\r' | '\n' | '\t' => '_',
            _ => c,
        })
        .collect()
}

fn json_escape(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_pair_str(key: &str, value: &str) -> String {
    format!("{}:{}", json_escape(key), json_escape(value))
}

fn json_pair_num(key: &str, value: u64) -> String {
    format!("{}:{}", json_escape(key), value)
}
