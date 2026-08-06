//! #26 - 100B packet-registry replay-prep: read-only loader + dry gate render.
//! Extracted from main.rs to honor the no-bloat invariant (<2000 LOC/file).
//! No fire: `auto_fire_allowed=0`, `process_launch=0` always. Child module of the crate
//! root, so it can use the root's private helpers directly.
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::{
    hbp_escape, hbp_field, query_param, render_shadow_parity, sha16, unix_seconds, Config, SeatBook,
};

fn parse_u128_param(query: &str, key: &str, fallback: u128) -> u128 {
    query_param(query, key)
        .and_then(|v| v.parse::<u128>().ok())
        .unwrap_or(fallback)
}

fn ceil_div_u128(n: u128, d: u128) -> u128 {
    if d == 0 {
        0
    } else {
        (n + d - 1) / d
    }
}

pub(crate) const REAL_100B_CHECKPOINT_FILE: &str = "checkpoint.state.json";
pub(crate) const REAL_100B_CHUNKS_FILE: &str = "real-100b-chunks.ndjson";

#[derive(Debug, Default)]
struct RegistryReplayStats {
    loaded: bool,
    error: String,
    path_sha16: String,
    checkpoint_processed: u128,
    checkpoint_target: u128,
    checkpoint_completed_chunks: u128,
    checkpoint_status: String,
    checkpoint_last_pid: String,
    chunks_seen: u128,
    chunks_loaded: u128,
    real_chunks: u128,
    accelerated_chunks: u128,
    other_chunks: u128,
    chunks_malformed: u128,
    packets_loaded: u128,
    genius_hits: u128,
    mistake_hits: u128,
    /// Packet-weighted averages as q (per-mille, 0..=1000); integer only.
    weighted_score_q: u32,
    weighted_reverse_gain_q: u32,
    first_chunk_hash: String,
    last_chunk_hash: String,
    promote_chunks: u128,
    hold_chunks: u128,
}

fn json_key_pos(text: &str, key: &str) -> Option<usize> {
    text.find(&format!("\"{}\"", key))
}

fn json_value_start(text: &str, key: &str) -> Option<usize> {
    let key_pos = json_key_pos(text, key)?;
    let after_key = &text[key_pos..];
    let colon_rel = after_key.find(':')?;
    Some(key_pos + colon_rel + 1)
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let mut i = json_value_start(text, key)?;
    let bytes = text.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'"' {
        return None;
    }
    i += 1;
    let start = i;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            return Some(text[start..i].to_string());
        }
        i += 1;
    }
    None
}

fn json_number_slice<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut i = json_value_start(text, key)?;
    let bytes = text.as_bytes();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    while i < bytes.len()
        && (bytes[i].is_ascii_digit()
            || bytes[i] == b'.'
            || bytes[i] == b'-'
            || bytes[i] == b'+'
            || bytes[i] == b'e'
            || bytes[i] == b'E')
    {
        i += 1;
    }
    if i == start {
        None
    } else {
        Some(&text[start..i])
    }
}

fn json_u128_field(text: &str, key: &str) -> Option<u128> {
    json_number_slice(text, key)?.parse::<u128>().ok()
}

/// Parse a JSON number in [0, 1] into q (per-mille, 0..=1000) WITHOUT float: the digit run is
/// read directly, three fractional digits are kept and the fourth rounds half-up. This replaces
/// `json_f64_field` + `q_from_unit`, and with it the platform-dependent rounding that the
/// registry test had to document (0.2524999.. rounding differently under MSVC).
fn json_unit_q_field(text: &str, key: &str) -> Option<u32> {
    let tok = json_number_slice(text, key)?.trim();
    if tok.is_empty() || tok.contains('e') || tok.contains('E') {
        return None;
    }
    let neg = tok.starts_with('-');
    let body = tok.trim_start_matches(['-', '+']);
    let (int_part, frac_part) = match body.split_once('.') {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };
    if !int_part.chars().all(|c| c.is_ascii_digit()) && !int_part.is_empty() {
        return None;
    }
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if neg {
        return Some(0);
    }
    let int_val: u32 = if int_part.is_empty() { 0 } else { int_part.parse().ok()? };
    if int_val >= 1 {
        return Some(1000);
    }
    let mut milli: u32 = 0;
    for (i, c) in frac_part.chars().take(4).enumerate() {
        let d = c.to_digit(10)?;
        if i < 3 {
            milli = milli * 10 + d;
        } else if d >= 5 {
            milli += 1;
        }
    }
    for _ in frac_part.chars().count()..3 {
        milli *= 10;
    }
    Some(milli.min(1000))
}

/// reverse_risk = 1 - reverse_gain, in q. Exact integer complement: 1000 - q.
pub(crate) fn reverse_risk_q_from_reverse_gain_q(avg_reverse_gain_q: u32) -> u32 {
    1000u32.saturating_sub(avg_reverse_gain_q.min(1000))
}

fn registry_path_from_query(query: &str) -> Option<String> {
    query_param(query, "registry")
        .filter(|v| !v.trim().is_empty())
        .or_else(|| env::var("ASOLARIA_100B_REGISTRY_DIR").ok())
        .map(|v| v.trim().to_string())
}

fn load_100b_registry_stats(registry_dir: &str, chunk_limit: Option<u128>) -> RegistryReplayStats {
    let mut stats = RegistryReplayStats {
        path_sha16: sha16(registry_dir),
        ..RegistryReplayStats::default()
    };
    let root = Path::new(registry_dir);
    let checkpoint_path = root.join(REAL_100B_CHECKPOINT_FILE);
    let chunks_path = root.join(REAL_100B_CHUNKS_FILE);

    let checkpoint = match fs::read_to_string(&checkpoint_path) {
        Ok(text) => text,
        Err(err) => {
            stats.error = format!("checkpoint_read_failed:{:?}", err.kind());
            return stats;
        }
    };
    stats.checkpoint_processed = json_u128_field(&checkpoint, "processedPackets").unwrap_or(0);
    stats.checkpoint_target = json_u128_field(&checkpoint, "targetPackets").unwrap_or(0);
    stats.checkpoint_completed_chunks =
        json_u128_field(&checkpoint, "completedChunks").unwrap_or(0);
    stats.checkpoint_status =
        json_string_field(&checkpoint, "status").unwrap_or_else(|| "missing".to_string());
    stats.checkpoint_last_pid =
        json_string_field(&checkpoint, "lastPacketPid").unwrap_or_else(|| "missing".to_string());

    let file = match fs::File::open(&chunks_path) {
        Ok(file) => file,
        Err(err) => {
            stats.error = format!("chunks_read_failed:{:?}", err.kind());
            return stats;
        }
    };

    // packet-weighted accumulators in q*packets; u128 so no overflow and no rounding
    let mut score_weight: u128 = 0;
    let mut rg_weight: u128 = 0;
    let mut weight_total: u128 = 0;
    let limit = chunk_limit.unwrap_or(u128::MAX);
    for line in BufReader::new(file).lines() {
        if stats.chunks_seen >= limit {
            break;
        }
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                stats.chunks_malformed += 1;
                continue;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        stats.chunks_seen += 1;
        let packets = json_u128_field(line, "packets").unwrap_or(0);
        let avg_score = json_unit_q_field(line, "avgScore");
        let avg_reverse_gain = json_unit_q_field(line, "avgReverseGain");
        if packets == 0 || avg_score.is_none() || avg_reverse_gain.is_none() {
            stats.chunks_malformed += 1;
            continue;
        }
        let avg_score = avg_score.unwrap();
        let avg_reverse_gain = avg_reverse_gain.unwrap();
        let kind = json_string_field(line, "kind").unwrap_or_else(|| "missing".to_string());
        match kind.as_str() {
            "real_100b_chunk" => stats.real_chunks += 1,
            "real_100b_accelerated_chunk" => stats.accelerated_chunks += 1,
            _ => stats.other_chunks += 1,
        }
        let genius = json_u128_field(line, "geniusHits").unwrap_or(0);
        let mistake = json_u128_field(line, "mistakeHits").unwrap_or(0);
        let chunk_hash = json_string_field(line, "chunkHash").unwrap_or_else(|| "-".to_string());
        if stats.first_chunk_hash.is_empty() {
            stats.first_chunk_hash = chunk_hash.clone();
        }
        stats.last_chunk_hash = chunk_hash;
        stats.chunks_loaded += 1;
        stats.packets_loaded += packets;
        stats.genius_hits += genius;
        stats.mistake_hits += mistake;
        let weight = packets;
        score_weight += avg_score as u128 * weight;
        rg_weight += avg_reverse_gain as u128 * weight;
        weight_total += weight;
        let score_q = avg_score;
        let risk_q = reverse_risk_q_from_reverse_gain_q(avg_reverse_gain);
        if score_q >= 720 && risk_q <= 280 {
            stats.promote_chunks += 1;
        } else {
            stats.hold_chunks += 1;
        }
    }
    if weight_total > 0 {
        // one integer divide with round-half-up, applied once at the end
        stats.weighted_score_q = ((score_weight * 2 + weight_total) / (weight_total * 2)) as u32;
        stats.weighted_reverse_gain_q = ((rg_weight * 2 + weight_total) / (weight_total * 2)) as u32;
    }
    stats.loaded = stats.chunks_loaded > 0;
    stats
}

/// `/replay-prep.hbp?target=N&sample=N&batch=N&device=&score=&risk=&registry=<dir>` — #26:
/// prepare the controlled 100B replay without running it. It samples the #25 shadow parity surface,
/// computes the batch geometry for the requested target, optionally reads the real 100B packet
/// registry (`checkpoint.state.json` + `real-100b-chunks.ndjson`) read-only, and emits hard gate
/// receipts. It is deliberately conservative: even if the sample and registry are clean,
/// `auto_fire_allowed=0` and `process_launch=0` remain.
pub(crate) fn render_replay_prep(
    book: &SeatBook,
    config: &Config,
    gnn_score_q: u32,
    query: &str,
) -> String {
    const DEFAULT_TARGET: u128 = 100_000_000_000;
    const DEFAULT_BATCH: u128 = 10_000;
    const MAX_SAMPLE: usize = 1000;

    let target = parse_u128_param(query, "target", DEFAULT_TARGET);
    let batch = parse_u128_param(query, "batch", DEFAULT_BATCH).max(1);
    let requested_sample = query_param(query, "sample")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);
    let sample = requested_sample.min(MAX_SAMPLE).min(book.seats.len());
    let device = query_param(query, "device").unwrap_or_else(|| "acer".to_string());
    let ts = query_param(query, "ts")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| unix_seconds().to_string());
    let role = query_param(query, "role").unwrap_or_default();
    let score = query_param(query, "score").unwrap_or_default();
    let risk = query_param(query, "risk").unwrap_or_default();
    let batches = ceil_div_u128(target, batch);
    let chunk_limit = query_param(query, "chunk_limit").and_then(|v| v.parse::<u128>().ok());

    let shadow_query = format!(
        "count={}&device={}&ts={}&role={}&score={}&risk={}",
        sample, device, ts, role, score, risk
    );
    let shadow = render_shadow_parity(book, config, gnn_score_q, &shadow_query);
    let shadow_head = shadow.lines().next().unwrap_or("");
    let matched = hbp_field(shadow_head, "matched").unwrap_or_else(|| "0".to_string());
    let mismatched = hbp_field(shadow_head, "mismatched").unwrap_or_else(|| "0".to_string());
    let proceed = hbp_field(shadow_head, "proceed").unwrap_or_else(|| "0".to_string());
    let hold = hbp_field(shadow_head, "hold").unwrap_or_else(|| "0".to_string());
    let block = hbp_field(shadow_head, "block").unwrap_or_else(|| "0".to_string());

    let status = if sample == 0 {
        "BLOCKED_NO_SEATS"
    } else if mismatched != "0" {
        "BLOCKED_SHADOW_MISMATCH"
    } else {
        "READY_FOR_PACKET_REGISTRY_REPLAY"
    };
    let reason = if sample == 0 {
        "seatbook_empty"
    } else if mismatched != "0" {
        "shadow_identity_mismatch"
    } else {
        "shadow_identity_clean_fire_still_gated"
    };

    let mut out = format!(
        "HOST8REPLAYPREP|target_total={}|batch_size={}|estimated_batches={}|sample_requested={}|sample_checked={}|sample_matched={}|sample_mismatched={}|sample_proceed={}|sample_hold={}|sample_block={}|status={}|reason={}|shadow_only=1|process_launch=0|auto_fire_allowed=0|operator_t0_required=1|json=0\n",
        target,
        batch,
        batches,
        requested_sample,
        sample,
        hbp_escape(&matched),
        hbp_escape(&mismatched),
        hbp_escape(&proceed),
        hbp_escape(&hold),
        hbp_escape(&block),
        status,
        reason
    );

    if let Some(registry_dir) = registry_path_from_query(query) {
        let stats = load_100b_registry_stats(&registry_dir, chunk_limit);
        let score_q = stats.weighted_score_q;
        let reverse_gain_q = stats.weighted_reverse_gain_q;
        let reverse_risk_q = reverse_risk_q_from_reverse_gain_q(stats.weighted_reverse_gain_q);
        let checkpoint_complete =
            stats.checkpoint_target > 0 && stats.checkpoint_processed >= stats.checkpoint_target;
        let packet_complete =
            stats.checkpoint_target > 0 && stats.packets_loaded >= stats.checkpoint_target;
        let chunks_complete = stats.checkpoint_completed_chunks > 0
            && stats.chunks_loaded >= stats.checkpoint_completed_chunks;
        let scalar_gate_clean = stats.loaded && stats.hold_chunks == 0;
        let registry_status = if !stats.error.is_empty() {
            "LOAD_ERROR"
        } else if checkpoint_complete && packet_complete && chunks_complete && scalar_gate_clean {
            "SCALAR_GATE_READY_SUPERVISOR_PIPELINE_REQUIRED"
        } else {
            "REVIEW_REQUIRED"
        };
        out.push_str(&format!(
            "HOST8REGISTRY|loaded={}|path_sha16={}|checkpoint_processed={}|checkpoint_target={}|checkpoint_completed_chunks={}|checkpoint_status={}|last_packet_pid={}|chunks_seen={}|chunks_loaded={}|real_chunks={}|accelerated_chunks={}|other_chunks={}|chunks_malformed={}|packets_loaded={}|genius_hits={}|mistake_hits={}|avg_score_q={}|avg_reverse_gain_q={}|reverse_risk_q={}|promote_chunks={}|hold_chunks={}|scalar_gate_clean={}|checkpoint_complete={}|packet_complete={}|chunks_complete={}|registry_status={}|error={}|first_chunk_hash={}|last_chunk_hash={}|reverse_risk_mapping=one_minus_reverse_gain|accepted_chunk_kinds=real_100b_chunk,real_100b_accelerated_chunk|process_launch=0|auto_fire_allowed=0|json=0\n",
            if stats.loaded { 1 } else { 0 },
            hbp_escape(&stats.path_sha16),
            stats.checkpoint_processed,
            stats.checkpoint_target,
            stats.checkpoint_completed_chunks,
            hbp_escape(&stats.checkpoint_status),
            hbp_escape(&stats.checkpoint_last_pid),
            stats.chunks_seen,
            stats.chunks_loaded,
            stats.real_chunks,
            stats.accelerated_chunks,
            stats.other_chunks,
            stats.chunks_malformed,
            stats.packets_loaded,
            stats.genius_hits,
            stats.mistake_hits,
            score_q,
            reverse_gain_q,
            reverse_risk_q,
            stats.promote_chunks,
            stats.hold_chunks,
            if scalar_gate_clean { 1 } else { 0 },
            if checkpoint_complete { 1 } else { 0 },
            if packet_complete { 1 } else { 0 },
            if chunks_complete { 1 } else { 0 },
            registry_status,
            hbp_escape(&stats.error),
            hbp_escape(&stats.first_chunk_hash),
            hbp_escape(&stats.last_chunk_hash),
        ));
        out.push_str("HOST8PIPELINE|required=gnn,hookwall,reverse_gain_gnn,whiteroom,omnishannon,shannon,omniflywheel|scalar_projection=1|pipeline_verified=0|status=SUPERVISOR_PIPELINE_REQUIRED|process_launch=0|auto_fire_allowed=0|json=0\n");
    } else {
        out.push_str(
            "HOST8REGISTRY|loaded=0|registry_status=NOT_REQUESTED|process_launch=0|auto_fire_allowed=0|json=0\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Config, Seat, SeatBook, DEFAULT_BIND, DEFAULT_FEED_DIR, DEFAULT_OFFICE_DIR,
        DEFAULT_OPENCODE_JS_BIN, DEFAULT_SUMMON_MODEL, DEFAULT_SUMMON_ROOT, DEFAULT_VERB_CATALOG,
    };

    // Minimal one-seat fixture (the registry assertions do not depend on seat identity).
    fn registry_test_fixture() -> (SeatBook, Config) {
        let mut book = SeatBook::default();
        book.seats.push(Seat {
            name: "t".to_string(),
            handle8: "0155964ffc8ef1f8".to_string(),
            cube_bh: "BH".to_string(),
            hilbert: "1339".to_string(),
            class: "level3_chief".to_string(),
            layer: "chief".to_string(),
            source: "test".to_string(),
        });
        book.verbs = vec!["VERB".to_string()];
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
    fn replay_prep_loads_100b_registry_fixture_readonly_and_maps_reverse_gain_to_risk() {
        let (book, config) = registry_test_fixture();
        let root =
            std::env::temp_dir().join(format!("asolaria-100b-registry-fixture-{}", unix_seconds()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(REAL_100B_CHECKPOINT_FILE),
            r#"{"processedPackets":200,"targetPackets":200,"completedChunks":2,"lastPacketPid":"BH.REAL100B.OPENCODE.PID.000000000200","status":"REAL_100B_PID_PACKET_RUN_COMPLETE"}"#,
        )
        .unwrap();
        std::fs::write(
            root.join(REAL_100B_CHUNKS_FILE),
            concat!(
                r#"{"kind":"real_100b_chunk","chunk":0,"packets":100,"geniusHits":91,"mistakeHits":9,"avgScore":0.91,"avgReverseGain":0.775,"chunkHash":"aaa111"}"#,
                "\n",
                r#"{"kind":"real_100b_accelerated_chunk","accelerationMode":"chunk_aggregate_sparse_proof","chunk":1,"packets":100,"geniusHits":80,"mistakeHits":20,"avgScore":0.80,"avgReverseGain":0.720,"chunkHash":"bbb222"}"#,
                "\n"
            ),
        )
        .unwrap();

        let query = format!(
            "target=200&batch=100&sample=1&device=acer&ts=1750000000&score=800&risk=10&registry={}",
            root.to_string_lossy()
        );
        let out = render_replay_prep(&book, &config, 0, &query);
        assert!(out.contains("HOST8REGISTRY|loaded=1"));
        assert!(out.contains("checkpoint_processed=200"));
        assert!(out.contains("checkpoint_target=200"));
        assert!(out.contains("checkpoint_completed_chunks=2"));
        assert!(out.contains("chunks_loaded=2"));
        assert!(out.contains("real_chunks=1"));
        assert!(out.contains("accelerated_chunks=1"));
        assert!(out.contains("other_chunks=0"));
        assert!(out.contains("packets_loaded=200"));
        assert!(out.contains("genius_hits=171"));
        assert!(out.contains("mistake_hits=29"));
        assert!(out.contains("avg_score_q=855"));
        // weighted avgReverseGain = 0.7475 -> q 748 (integer round-half-up); reverse_risk is
        // the exact complement 1000 - 748 = 252 on every platform. The old float path produced
        // 0.2524999.. and depended on the host's rounding — that hazard is gone.
        assert!(out.contains("avg_reverse_gain_q=748"));
        assert!(out.contains("reverse_risk_q=252"));
        assert!(out.contains("promote_chunks=2"));
        assert!(out.contains("hold_chunks=0"));
        assert!(out.contains("scalar_gate_clean=1"));
        assert!(out.contains("checkpoint_complete=1"));
        assert!(out.contains("packet_complete=1"));
        assert!(out.contains("chunks_complete=1"));
        assert!(out.contains("registry_status=SCALAR_GATE_READY_SUPERVISOR_PIPELINE_REQUIRED"));
        assert!(out.contains("reverse_risk_mapping=one_minus_reverse_gain"));
        assert!(out.contains("accepted_chunk_kinds=real_100b_chunk,real_100b_accelerated_chunk"));
        assert!(out.contains("HOST8PIPELINE|"));
        assert!(out.contains(
            "required=gnn,hookwall,reverse_gain_gnn,whiteroom,omnishannon,shannon,omniflywheel"
        ));
        assert!(out.contains("pipeline_verified=0"));
        assert!(out.contains("status=SUPERVISOR_PIPELINE_REQUIRED"));
        assert!(out.contains("process_launch=0"));
        assert!(out.contains("auto_fire_allowed=0"));
        assert!(out.contains("json=0"));
        assert!(!out.contains('{'));
        assert!(!out.contains('}'));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reverse_gain_to_reverse_risk_mapping_matches_omniflywheel_gate() {
        assert_eq!(reverse_risk_q_from_reverse_gain_q(0.775), 225);
        assert_eq!(reverse_risk_q_from_reverse_gain_q(0.720), 280);
        assert_eq!(reverse_risk_q_from_reverse_gain_q(0.719), 281);
    }
}
