use std::collections::BTreeMap;
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use asolaria_server_fischer_eval::{
    evaluate, hbp_escape, parse_bool, parse_f64, render_hbi, render_row, sha8, FischerEnvelope,
    FischerScore,
};

const DEFAULT_BIND: &str = "127.0.0.1:5089";
const ENGINE_ID: &str = "fischer-eval-host8";
const LIVE_COMPAT: &str = "fischer-live-4794";

fn main() {
    let config = Config::parse(env::args().skip(1));
    if config.once {
        print!("{}", render_health());
        return;
    }
    if let Some(input) = config.eval_input.as_deref() {
        print!("{}", render_eval_input(input));
        return;
    }

    let listener = TcpListener::bind(&config.bind).unwrap_or_else(|error| {
        eprintln!(
            "FISCHERERR|error=bind_failed|bind={}|detail={}|json=0",
            hbp_escape(&config.bind),
            hbp_escape(&error.to_string())
        );
        process::exit(2);
    });
    println!(
        "FISCHERLISTEN|engine={}|bind={}|live_compat={}|json=0",
        ENGINE_ID, config.bind, LIVE_COMPAT
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_client(stream),
            Err(error) => eprintln!(
                "FISCHERERR|error=accept_failed|detail={}|json=0",
                hbp_escape(&error.to_string())
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct Config {
    bind: String,
    once: bool,
    eval_input: Option<String>,
}

impl Config {
    fn parse<I>(args: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let args: Vec<String> = args.into_iter().collect();
        let mut config = Self {
            bind: env::var("FISCHER_EVAL_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string()),
            once: false,
            eval_input: None,
        };
        let mut i = 0usize;
        while i < args.len() {
            match args[i].as_str() {
                "--bind" => {
                    if let Some(value) = args.get(i + 1) {
                        config.bind = value.clone();
                        i += 1;
                    }
                }
                "--once" => config.once = true,
                "--eval" => {
                    if let Some(value) = args.get(i + 1) {
                        config.eval_input = Some(value.clone());
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        config
    }
}

fn handle_client(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() || request.is_empty() {
        return;
    }

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header == "\r\n" || header == "\n" {
            break;
        }
        let lower = header.to_ascii_lowercase();
        if let Some(raw) = lower.strip_prefix("content-length:") {
            content_length = raw.trim().parse::<usize>().unwrap_or(0);
        }
    }

    let mut body = String::new();
    if content_length > 0 && content_length <= 256 * 1024 {
        let mut buf = vec![0u8; content_length];
        if reader.read_exact(&mut buf).is_ok() {
            body = String::from_utf8_lossy(&buf).to_string();
        }
    }

    let method = request.split_whitespace().next().unwrap_or("GET");
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (status, response_body) = match (method, path) {
        ("GET", "/") | ("GET", "/health.hbp") | ("GET", "/health") => ("200 OK", render_health()),
        ("GET", "/spec.hbp") => ("200 OK", render_spec()),
        ("GET", "/eval.hbp") => ("200 OK", render_eval_input(query)),
        ("POST", "/fischer/eval") | ("POST", "/eval.hbp") => {
            ("200 OK", render_eval_input(body.trim()))
        }
        ("GET", "/hbi.hbp") => ("200 OK", render_hbi_input(query)),
        _ => (
            "404 Not Found",
            format!(
                "FISCHERERR|status=404|error=unknown_route|path={}|json=0|row_hash={}\n",
                hbp_escape(path),
                sha8(path)
            ),
        ),
    };
    respond(&mut stream, status, &response_body);
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    );
}

fn render_health() -> String {
    format!(
        "FISCHERHOST8|ok=1|engine={}|live_compat={}|pid={}|routes=/health.hbp,/spec.hbp,/eval.hbp,/fischer/eval|pipeline=VERIFY-FISCHER-HOOKWALL-ROUTE|self_authorizes=0|cosign_appends=0|json=0\n",
        ENGINE_ID,
        LIVE_COMPAT,
        process::id()
    )
}

fn render_spec() -> String {
    [
        "FISCHERSPEC|id=BHFISCHER-KERNEL-v1|formula=cpl_penalties_minus_gains_with_hard_floors|pipeline=VERIFY-FISCHER-EVAL-HOOKWALL-ROUTE|json=0\n",
        "FISCHERTHRESH|proceed_cpl_lt=150|hold_cpl_gte=150|block_cpl_gte=500|refute=known_bad_pattern|analyze=write_or_spawn_without_replay_path|json=0\n",
        "FISCHERROLE|multi_level=1|levels=op,council,supervisor,agent,route,omni-system|self_authorizes=0|cosign_appends=0|json=0\n",
        "FISCHERFIELDS|required=pid,move,verdict,cpl,candidate_count,best_alt,king_safety,center_gain,proof_gain,authority_debt,g4_state,voxel_coord,glyph,flags,l0_real,composite,prev_hash,ts,json,runtime,row_hash|json=0\n",
    ].concat()
}

fn render_eval_input(input: &str) -> String {
    let fields = parse_input_fields(input);
    let (envelope, score, strict, prev_hash) = fields_to_eval(&fields);
    let eval = evaluate(&envelope, &score, strict);
    render_row(&eval, &prev_hash, &isoish_ts())
}

fn render_hbi_input(input: &str) -> String {
    let fields = parse_input_fields(input);
    let (envelope, score, strict, _) = fields_to_eval(&fields);
    let eval = evaluate(&envelope, &score, strict);
    render_hbi(&eval)
}

fn fields_to_eval(
    fields: &BTreeMap<String, String>,
) -> (FischerEnvelope, FischerScore, bool, String) {
    let mut env = FischerEnvelope::default();
    env.pid = opt_nonempty(fields, "pid")
        .or_else(|| opt_nonempty(fields, "bh"))
        .or(env.pid);
    env.actor = val(fields, "actor", &env.actor);
    env.verb = val_alias(fields, &["verb", "move"], &env.verb);
    env.target = val(fields, "target", &env.target);
    env.payload = val_alias(fields, &["payload", "content"], &env.payload);
    env.payload_json_true = parse_bool(opt_str_alias(fields, &["payload_json", "json"]).as_deref());
    env.hbp_path = opt_nonempty(fields, "hbp_path");
    env.sidecar_plan = opt_nonempty(fields, "sidecar_plan");
    env.ledger_path = opt_nonempty(fields, "ledger_path");
    env.tuple = opt_nonempty(fields, "tuple");
    env.cube_47d = opt_nonempty(fields, "cube_47d");
    env.cosign = opt_nonempty(fields, "cosign");
    env.halt_path = opt_nonempty(fields, "halt_path");
    env.authority_jump = parse_bool(opt_str(fields, "authority_jump").as_deref());
    env.recursive_consent = parse_bool(opt_str(fields, "recursive_consent").as_deref());
    env.operator_witness_required =
        parse_bool(opt_str(fields, "operator_witness_required").as_deref());
    env.operator_witness = parse_bool(opt_str(fields, "operator_witness").as_deref());

    let score = FischerScore {
        composite: val(fields, "composite", "0"),
        l0_real: parse_bool(opt_str(fields, "l0_real").as_deref()),
        shannon: parse_f64(opt_str(fields, "shannon").as_deref(), 0.0),
        g4_state: val(fields, "g4_state", "UNKNOWN"),
    };
    let strict = parse_bool(opt_str(fields, "strict").as_deref());
    let prev_hash = val(fields, "prev_hash", "0000000000000000");
    (env, score, strict, prev_hash)
}

fn parse_input_fields(input: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let cleaned = input.trim();
    let sep = if cleaned.contains('|') { '|' } else { '&' };
    for (idx, part) in cleaned.split(sep).enumerate() {
        if idx == 0 && sep == '|' && !part.contains('=') {
            fields.insert("kind".to_string(), part.to_string());
            continue;
        }
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        if key.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), percent_decode(value));
    }
    fields
}

fn val(fields: &BTreeMap<String, String>, key: &str, default: &str) -> String {
    fields
        .get(key)
        .filter(|v| !v.is_empty())
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn val_alias(fields: &BTreeMap<String, String>, keys: &[&str], default: &str) -> String {
    for key in keys {
        if let Some(v) = fields.get(*key).filter(|v| !v.is_empty()) {
            return v.clone();
        }
    }
    default.to_string()
}

fn opt_str(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields.get(key).cloned()
}

fn opt_str_alias(fields: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = fields.get(*key) {
            return Some(v.clone());
        }
    }
    None
}

fn opt_nonempty(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields.get(key).filter(|v| !v.is_empty()).cloned()
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((a * 16 + b) as char);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { ' ' } else { bytes[i] as char });
        i += 1;
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn isoish_ts() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_route_is_hbp_and_no_json() {
        let out = render_eval_input(
            "pid=BH.HOOKWALL.TEST000000000001&verb=tick&target=fabric&hbp_path=x&l0_real=1&tuple=BH.HOOKWALL.TEST000000000001&composite=0.85",
        );
        assert!(out.contains("FISCHERv1|"));
        assert!(out.contains("|move=tick|"));
        assert!(out.contains("|verdict=PROCEED|"));
        assert!(out.contains("|json=0|"));
        assert!(!out.contains('{'));
        assert!(!out.contains("|verb="));
    }

    #[test]
    fn self_authorize_refutes() {
        let out = render_eval_input("pid=BH.X&verb=self_authorize&target=fabric&hbp_path=x");
        assert!(out.contains("|verdict=REFUTE|"));
        assert!(out.contains("|cpl=999|"));
    }

    #[test]
    fn spec_declares_non_authorizing_role() {
        let out = render_spec();
        assert!(out.contains("FISCHERROLE|"));
        assert!(out.contains("self_authorizes=0"));
        assert!(out.contains("cosign_appends=0"));
        assert!(!out.contains('{'));
    }
}
