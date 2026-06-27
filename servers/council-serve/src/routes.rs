//! Read-only json=0 routes. The centerpiece is `/api/vote-quorum/parity` — the on-disk vote-ledger
//! parity harness liris asked for: recompute each row's row_hash via the vote-quorum `canon` recipe
//! and compare to the stored value (proves the Rust canon == the py daemon `append_row`). The live
//! ledger files are NEVER written. No append/cutover/fire route exists in this build.

use std::fs;
use std::sync::Arc;

use asolaria_server_vote_quorum::canon;

use crate::schedule;
use crate::Shared;

fn hbp_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '|' | '\r' | '\n' | '\t' => '_',
            _ => c,
        })
        .collect()
}

pub fn route(shared: &Arc<Shared>, method: &str, path: &str, _query: &str) -> (u16, String) {
    match (method, path) {
        ("GET", "/health") => (
            200,
            String::from("COUNCILSERVE|ok=1|service=asolaria-council-serve|port=5090|engine=gated_own_thread|process_launch=0|auto_fire=false|read_only=true|json=0\n"),
        ),
        ("GET", "/api/vote-quorum/parity") => parity(shared),
        ("GET", "/api/council/status") => (
            200,
            String::from("COUNCILSTATUS|ok=1|responder=live|engine=gated|loop_routes=read_only_staged|cutover=false|json=0\n"),
        ),
        ("GET", "/api/loop/pending") => (
            200,
            String::from("LOOPPENDING|ok=1|status=staged|read_only=true|pending_count=unknown|source=loop_ledger_unwired|auto_fire_allowed=false|cutover=false|json=0\n"),
        ),
        ("GET", "/api/loop/tick") => (
            200,
            String::from("LOOPTICK|ok=1|status=staged|read_only=true|tick_executed=false|process_launch=0|auto_fire=false|spawner_emit=false|cutover=false|json=0\n"),
        ),
        ("GET", "/api/loop/veto") => (
            200,
            String::from("LOOPVETO|ok=1|status=staged|read_only=true|veto_submitted=false|operator_witness_required=true|cutover=false|json=0\n"),
        ),
        ("GET", "/api/loop/schedule") => schedule_route(),
        _ => (
            404,
            format!("COUNCILSERVE|ok=0|error=unknown_route|path={}|json=0\n", hbp_escape(path)),
        ),
    }
}

/// Recompute each on-disk vote-ledger row_hash via the canon recipe; compare to stored. Read-only.
fn parity(shared: &Arc<Shared>) -> (u16, String) {
    let mut out = String::new();
    let mut ok = true;
    for name in ["queue", "votes", "outcomes"] {
        let p = shared.vote_dir.join(format!("{name}.ndjson"));
        let (mut total, mut matched, mut mismatch, mut legacy) = (0u64, 0u64, 0u64, 0u64);
        match fs::read_to_string(&p) {
            Ok(body) => {
                for line in body.lines() {
                    let t = line.trim();
                    if t.is_empty() {
                        continue;
                    }
                    match canon::stored_row_hash(t) {
                        None => legacy += 1,
                        Some(stored) => {
                            total += 1;
                            match canon::recompute(t) {
                                Ok(got) if got == stored => matched += 1,
                                _ => mismatch += 1,
                            }
                        }
                    }
                }
                out.push_str(&format!(
                    "COUNCILPARITY|ledger={}|ok=1|missing=0|rows_with_hash={}|match={}|mismatch={}|legacy={}|json=0\n",
                    name, total, matched, mismatch, legacy
                ));
            }
            Err(e) => {
                ok = false;
                out.push_str(&format!(
                    "COUNCILPARITY|ledger={}|ok=0|missing=1|error=ledger_unreadable|kind={}|json=0\n",
                    name,
                    hbp_escape(&e.kind().to_string())
                ));
            }
        }
    }
    (if ok { 200 } else { 500 }, out)
}

/// Read-only confidence-scheduled-verify route (the DSpark lesson applied to the crank). Reads the
/// loop ledger (ndjson of proposals) from `ASOLARIA_LOOP_LEDGER`; if unset/empty -> STAGED (ledger
/// unwired), never a fake clean schedule. Verify budget / accept threshold are env-tunable. STAGED:
/// it computes verify INTENT only and NEVER fires.
fn schedule_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_LOOP_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("COUNCILSCHEDULE|ok=1|status=staged|read_only=true|source=loop_ledger_unwired|hint=set_ASOLARIA_LOOP_LEDGER|scheduler=confidence_scheduled_verify|fire=false|cutover=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "COUNCILSCHEDULE|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_schedule(
        &body,
        env_usize("ASOLARIA_VERIFY_BUDGET", 8),
        env_f64("ASOLARIA_ACCEPT_THRESHOLD", 0.5),
    )
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Pure: parse an ndjson loop ledger, run the confidence schedule, render json=0 HBP rows. Never fires.
fn render_schedule(body: &str, budget: usize, threshold: f64) -> (u16, String) {
    let mut proposals = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_proposal(t) {
            Some(p) => proposals.push(p),
            None => legacy += 1,
        }
    }
    let sched = schedule::confidence_schedule(&proposals, budget, threshold);
    let verify_n = sched.iter().filter(|s| s.verify).count();
    let mut out = format!(
        "COUNCILSCHEDULE|ok=1|status=staged|read_only=true|scheduler=confidence_scheduled_verify|total={}|verify={}|defer={}|legacy={}|budget={}|threshold={:.3}|fire=false|cutover=false|json=0\n",
        proposals.len(),
        verify_n,
        proposals.len().saturating_sub(verify_n),
        legacy,
        budget,
        threshold
    );
    for s in &sched {
        out.push_str(&format!(
            "COUNCILSCHED|rank={}|id={}|confidence={:.4}|action={}|fire=false|json=0\n",
            s.rank,
            hbp_escape(&s.id),
            s.confidence,
            if s.verify { "VERIFY" } else { "DEFER" }
        ));
    }
    (200, out)
}

/// Parse one ndjson loop-ledger line into a Proposal. Requires `id` + `score` (GNN confidence);
/// `genius` falls back to `score` (reverse-gain not yet run), `risk` to 0.0. Unparseable -> None.
fn parse_proposal(line: &str) -> Option<schedule::Proposal> {
    let id = json_str(line, "id")?;
    let score = json_num(line, "score")?;
    let genius = json_num(line, "genius").unwrap_or(score);
    let risk = json_num(line, "risk").unwrap_or(0.0);
    Some(schedule::Proposal {
        id,
        score,
        genius,
        risk,
    })
}

fn json_str(s: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)? + pat.len();
    let after = s[i..].trim_start_matches([' ', ':']);
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn json_num(s: &str, key: &str) -> Option<f64> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)? + pat.len();
    let after = s[i..].trim_start_matches([' ', ':']);
    let end = after
        .find(|c: char| {
            !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')
        })
        .unwrap_or(after.len());
    after[..end].parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn health_is_json0_and_gated() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/health", "");
        assert_eq!(code, 200);
        assert!(body.contains("json=0"));
        assert!(body.contains("auto_fire=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn unknown_route_is_404_json0() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/nope", "");
        assert_eq!(code, 404);
        assert!(body.ends_with("json=0\n"));
    }

    #[test]
    fn no_append_or_fire_route() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        for p in [
            "/api/vote-quorum/cast",
            "/api/vote-quorum/submit",
            "/summon",
            "/fire",
        ] {
            assert_eq!(
                route(&sh, "POST", p, "fire=1").0,
                404,
                "no write/fire route: {p}"
            );
        }
    }

    #[test]
    fn loop_routes_are_read_only_staged_json0() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        for p in ["/api/loop/pending", "/api/loop/tick", "/api/loop/veto"] {
            let (code, body) = route(&sh, "GET", p, "");
            assert_eq!(code, 200, "{p}");
            assert!(body.contains("read_only=true"), "{p}: {body}");
            assert!(body.contains("cutover=false"), "{p}: {body}");
            assert!(body.ends_with("json=0\n"), "{p}: {body}");
            assert!(!body.contains('{'), "{p}: {body}");
        }
    }

    #[test]
    fn mutating_loop_routes_are_not_wired() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        for p in ["/api/loop/pending", "/api/loop/tick", "/api/loop/veto"] {
            let (code, body) = route(&sh, "POST", p, "");
            assert_eq!(code, 404, "{p}");
            assert!(body.contains("error=unknown_route"));
            assert!(body.ends_with("json=0\n"));
        }
    }

    #[test]
    fn parity_missing_ledgers_are_not_clean_zeroes() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("__asolaria_missing_vote_dir_for_test__"),
        });
        let (code, body) = route(&sh, "GET", "/api/vote-quorum/parity", "");
        assert_eq!(code, 500);
        assert!(body.contains("ok=0"));
        assert!(body.contains("missing=1"));
        assert!(body.contains("error=ledger_unreadable"));
        assert!(!body.contains("rows_with_hash=0|match=0|mismatch=0|legacy=0"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn parity_empty_existing_ledgers_are_clean_zeroes() {
        let dir = std::env::temp_dir().join(format!(
            "asolaria-council-serve-empty-ledgers-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["queue", "votes", "outcomes"] {
            std::fs::write(dir.join(format!("{name}.ndjson")), "").unwrap();
        }
        let sh = Arc::new(Shared {
            vote_dir: dir.clone(),
        });
        let (code, body) = route(&sh, "GET", "/api/vote-quorum/parity", "");
        assert_eq!(code, 200);
        assert_eq!(body.matches("ok=1|missing=0").count(), 3);
        assert_eq!(
            body.matches("rows_with_hash=0|match=0|mismatch=0|legacy=0")
                .count(),
            3
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod schedule_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_proposal_reads_score_and_defaults_genius_to_score() {
        let p = parse_proposal(r#"{"id":"x1","score":0.8,"risk":0.1}"#).unwrap();
        assert_eq!(p.id, "x1");
        assert!((p.score - 0.8).abs() < 1e-9);
        assert!((p.genius - 0.8).abs() < 1e-9); // genius falls back to score
        assert!((p.risk - 0.1).abs() < 1e-9);
        assert!(parse_proposal(r#"{"no_id":true}"#).is_none());
        assert!(parse_proposal("not json").is_none());
    }

    #[test]
    fn render_schedule_rations_verify_and_never_fires() {
        let ledger = "{\"id\":\"hi\",\"score\":0.9,\"genius\":0.9}\n{\"id\":\"lo\",\"score\":0.1,\"genius\":0.1}\n";
        let (code, body) = render_schedule(ledger, 1, 0.5);
        assert_eq!(code, 200);
        assert!(body.contains("scheduler=confidence_scheduled_verify"));
        assert!(body.contains("total=2"));
        assert!(body.contains("verify=1"));
        assert!(body.contains("action=VERIFY"));
        assert!(body.contains("action=DEFER"));
        assert!(body.contains("fire=false"));
        assert!(!body.contains('{'));
        // the high-survival proposal is the one ranked first
        assert!(body.contains("rank=0|id=hi"));
    }

    #[test]
    fn render_schedule_empty_ledger_is_clean_zero_not_faked() {
        let (code, body) = render_schedule("", 8, 0.5);
        assert_eq!(code, 200);
        assert!(body.contains("total=0"));
        assert!(body.contains("verify=0"));
        assert!(body.contains("fire=false"));
    }

    #[test]
    fn schedule_route_is_wired_read_only_and_never_fires() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/loop/schedule", "");
        assert_ne!(code, 404, "schedule route must be wired");
        assert!(!body.contains("error=unknown_route"));
        // staged or rendered branch -> fire=false; unreadable-ledger branch -> ledger_unreadable. Never fires.
        assert!(body.contains("fire=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn no_post_fire_on_schedule_route() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        assert_eq!(route(&sh, "POST", "/api/loop/schedule", "fire=1").0, 404);
    }
}
