//! Read-only json=0 routes. The centerpiece is `/api/vote-quorum/parity` — the on-disk vote-ledger
//! parity harness liris asked for: recompute each row's row_hash via the vote-quorum `canon` recipe
//! and compare to the stored value (proves the Rust canon == the py daemon `append_row`). The live
//! ledger files are NEVER written. No append/cutover/fire route exists in this build.

use std::fs;
use std::sync::Arc;

use asolaria_server_vote_quorum::canon;

use crate::lane_health;
use crate::policy;
use crate::schedule;
use crate::stale_branch;
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
        ("GET", "/api/lane/health") => lane_health_route(),
        ("GET", "/api/policy/evaluate") => policy_route(),
        ("GET", "/api/branch/freshness") => branch_freshness_route(),
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

fn json_bool(s: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{key}\"");
    let i = s.find(&pat)? + pat.len();
    let after = s[i..].trim_start_matches([' ', ':']);
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Read-only lane-health route (absorbed claw-code worker-lifecycle 6-way classifier). Reads a lane
/// evidence ledger (ndjson) from `ASOLARIA_LANE_LEDGER`; unset/empty -> staged (never fake-clean).
/// Diagnoses each lane's stall reason / ready-but-idle / ok. Diagnoses only — never spawns or fires.
fn lane_health_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_LANE_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("LANEHEALTH|ok=1|status=staged|read_only=true|source=lane_ledger_unwired|hint=set_ASOLARIA_LANE_LEDGER|classifier=worker_lifecycle_6way|fire=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "LANEHEALTH|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_lane_health(
        &body,
        env_usize("ASOLARIA_ACCEPT_TIMEOUT_MS", 90_000) as u64,
    )
}

/// Parse one ndjson lane-evidence row -> LaneEvidence. Requires `id`; other fields default safe
/// (transport_ok=true, flags false, elapsed_ms=0). Unparseable (no id) -> None.
fn parse_lane(line: &str) -> Option<lane_health::LaneEvidence> {
    let id = json_str(line, "id")?;
    Some(lane_health::LaneEvidence {
        id,
        state: lane_health::WorkerState::parse(&json_str(line, "state").unwrap_or_default()),
        transport_ok: json_bool(line, "transport_ok").unwrap_or(true),
        trust_prompt_detected: json_bool(line, "trust").unwrap_or(false),
        prompt_sent: json_bool(line, "prompt_sent").unwrap_or(false),
        prompt_accepted: json_bool(line, "prompt_accepted").unwrap_or(false),
        misdelivered: json_bool(line, "misdelivered").unwrap_or(false),
        crashed: json_bool(line, "crashed").unwrap_or(false),
        elapsed_ms: json_num(line, "elapsed_ms").unwrap_or(0.0) as u64,
        terminal: json_bool(line, "terminal").unwrap_or(false),
    })
}

/// Pure: parse the lane ledger, diagnose each lane, render json=0 HBP. Never fires.
fn render_lane_health(body: &str, acceptance_timeout_ms: u64) -> (u16, String) {
    let mut lanes = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_lane(t) {
            Some(l) => lanes.push(l),
            None => legacy += 1,
        }
    }
    let diags: Vec<_> = lanes
        .iter()
        .map(|l| lane_health::diagnose(l, acceptance_timeout_ms))
        .collect();
    let stalled = diags.iter().filter(|d| d.failure.is_some()).count();
    let idle = diags.iter().filter(|d| d.ready_but_idle).count();
    let unique_done = lane_health::count_unique_terminal(&lanes);
    let mut out = format!(
        "LANEHEALTH|ok=1|status=staged|read_only=true|classifier=worker_lifecycle_6way|total={}|stalled={}|ready_but_idle={}|unique_terminal={}|legacy={}|timeout_ms={}|fire=false|json=0\n",
        lanes.len(),
        stalled,
        idle,
        unique_done,
        legacy,
        acceptance_timeout_ms
    );
    for d in &diags {
        let verdict = match &d.failure {
            Some(f) => f.as_str(),
            None if d.ready_but_idle => "ready_but_idle",
            None => "ok",
        };
        out.push_str(&format!(
            "LANE|id={}|verdict={}|json=0\n",
            hbp_escape(&d.id),
            verdict
        ));
    }
    (200, out)
}

/// Read-only policy-engine route (absorbed claw-code policy engine = the executable PR-triage). Reads
/// a PR-context ledger (ndjson) from `ASOLARIA_PR_LEDGER`; unset/empty -> staged. Evaluates each PR's
/// disposition (merge/close/rebase/hold). DECIDES ONLY — performs no merge/close/rebase/fire.
fn policy_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_PR_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("POLICY|ok=1|status=staged|read_only=true|source=pr_ledger_unwired|hint=set_ASOLARIA_PR_LEDGER|engine=pr_triage_policy|execute=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "POLICY|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_policy(&body)
}

/// Parse one ndjson PR-context row -> PrContext. Requires `id`; flags default safe (ci_ok=true so a
/// missing CI gate doesn't block; everything else false -> bare rows fall to HOLD). Unparseable -> None.
fn parse_pr_context(line: &str) -> Option<policy::PrContext> {
    let id = json_str(line, "id")?;
    Some(policy::PrContext {
        id,
        ci_ok: json_bool(line, "ci_ok").unwrap_or(true),
        clean: json_bool(line, "clean").unwrap_or(false),
        dirty: json_bool(line, "dirty").unwrap_or(false),
        draft: json_bool(line, "draft").unwrap_or(false),
        superseded: json_bool(line, "superseded").unwrap_or(false),
        additive: json_bool(line, "additive").unwrap_or(false),
        sensitive: json_bool(line, "sensitive").unwrap_or(false),
        author_forbids: json_bool(line, "author_forbids").unwrap_or(false),
    })
}

/// Pure: parse the PR ledger, evaluate each disposition, render json=0 HBP. Decides only; never executes.
fn render_policy(body: &str) -> (u16, String) {
    let mut prs = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_pr_context(t) {
            Some(p) => prs.push(p),
            None => legacy += 1,
        }
    }
    let decided: Vec<_> = prs
        .iter()
        .map(|p| (p.id.clone(), policy::evaluate(p)))
        .collect();
    let count = |a: policy::PolicyAction| decided.iter().filter(|(_, (act, _))| *act == a).count();
    let mut out = format!(
        "POLICY|ok=1|status=staged|read_only=true|engine=pr_triage_policy|total={}|merge={}|close={}|rebase={}|hold={}|legacy={}|execute=false|json=0\n",
        prs.len(),
        count(policy::PolicyAction::Merge),
        count(policy::PolicyAction::Close),
        count(policy::PolicyAction::Rebase),
        count(policy::PolicyAction::Hold),
        legacy
    );
    for (id, (action, reason)) in &decided {
        out.push_str(&format!(
            "PRPOLICY|id={}|action={}|reason={}|execute=false|json=0\n",
            hbp_escape(id),
            action.as_str(),
            hbp_escape(reason)
        ));
    }
    (200, out)
}

/// Read-only branch-freshness route (absorbed claw-code stale-branch detection). Reads a branch
/// ledger (ndjson) from `ASOLARIA_BRANCH_LEDGER`; unset/empty -> staged. Assesses each branch's
/// freshness (fresh/stale/diverged) + the refresh action (ready/merge_forward/rebase/block) and
/// flags stale-noise (RED on a non-fresh branch ≠ a new regression). RECOMMENDS ONLY — it never
/// rebases, merges, or fires.
fn branch_freshness_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_BRANCH_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("BRANCHFRESH|ok=1|status=staged|read_only=true|source=branch_ledger_unwired|hint=set_ASOLARIA_BRANCH_LEDGER|detector=stale_branch_freshness|fire=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "BRANCHFRESH|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_branch_freshness(&body)
}

/// One branch row parsed from the ledger: id + its ahead/behind counts vs base + conflicting flag.
struct BranchRow {
    id: String,
    ahead_by: Option<u64>,
    behind_by: Option<u64>,
    conflicting: Option<bool>,
}

/// Parse one ndjson branch row -> BranchRow. Requires `id`; missing counts/conflict are preserved as
/// unknown so the detector fails closed instead of manufacturing `fresh -> ready`.
fn parse_branch(line: &str) -> Option<BranchRow> {
    let id = json_str(line, "id")?;
    Some(BranchRow {
        id,
        ahead_by: json_nonnegative_u64(line, "ahead_by"),
        behind_by: json_nonnegative_u64(line, "behind_by"),
        conflicting: json_bool(line, "conflicting"),
    })
}

fn json_nonnegative_u64(s: &str, key: &str) -> Option<u64> {
    let n = json_num(s, key)?;
    if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= u64::MAX as f64 {
        Some(n as u64)
    } else {
        None
    }
}

/// Pure: parse the branch ledger, assess freshness + recommend the refresh action per branch,
/// render json=0 HBP. Recommends only; never rebases/merges/fires.
fn render_branch_freshness(body: &str) -> (u16, String) {
    let mut rows = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_branch(t) {
            Some(r) => rows.push(r),
            None => legacy += 1,
        }
    }
    let assessed: Vec<_> = rows
        .iter()
        .map(|r| {
            let fresh = stale_branch::assess(r.ahead_by, r.behind_by);
            let (action, reason) = stale_branch::recommend(fresh, r.conflicting);
            (r, fresh, action, reason)
        })
        .collect();
    let count_action =
        |a: stale_branch::RefreshAction| assessed.iter().filter(|(_, _, act, _)| *act == a).count();
    let stale_noise = assessed
        .iter()
        .filter(|(_, f, _, _)| stale_branch::is_stale_noise(*f))
        .count();
    let mut out = format!(
        "BRANCHFRESH|ok=1|status=staged|read_only=true|detector=stale_branch_freshness|total={}|ready={}|merge_forward={}|rebase={}|block={}|stale_noise={}|legacy={}|fire=false|json=0\n",
        rows.len(),
        count_action(stale_branch::RefreshAction::Ready),
        count_action(stale_branch::RefreshAction::MergeForward),
        count_action(stale_branch::RefreshAction::Rebase),
        count_action(stale_branch::RefreshAction::Block),
        stale_noise,
        legacy
    );
    for (r, fresh, action, reason) in &assessed {
        out.push_str(&format!(
            "BRANCH|id={}|freshness={}|action={}|stale_noise={}|reason={}|fire=false|json=0\n",
            hbp_escape(&r.id),
            fresh.as_str(),
            action.as_str(),
            stale_branch::is_stale_noise(*fresh),
            hbp_escape(reason)
        ));
    }
    (200, out)
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

#[cfg(test)]
mod lane_health_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_lane_requires_id_and_defaults_safe() {
        let l = parse_lane(r#"{"id":"L1","state":"ready_for_prompt"}"#).unwrap();
        assert_eq!(l.id, "L1");
        assert!(l.transport_ok); // defaults true
        assert!(!l.prompt_sent);
        assert!(parse_lane(r#"{"no_id":true}"#).is_none());
    }

    #[test]
    fn render_lane_health_classifies_and_dedupes_terminals() {
        let ledger = [
            r#"{"id":"idle1","state":"ready_for_prompt","elapsed_ms":999999}"#,
            r#"{"id":"to1","state":"ready_for_prompt","prompt_sent":true,"elapsed_ms":999999}"#,
            r#"{"id":"dead1","state":"spawning","transport_ok":false}"#,
            r#"{"id":"run1","state":"running","elapsed_ms":999999}"#,
            r#"{"id":"done1","state":"finished","terminal":true}"#,
            r#"{"id":"done1","state":"finished","terminal":true}"#,
        ]
        .join("\n");
        let (code, body) = render_lane_health(&ledger, 90_000);
        assert_eq!(code, 200);
        assert!(body.contains("classifier=worker_lifecycle_6way"));
        assert!(body.contains("total=6"));
        assert!(body.contains("stalled=2"), "{body}");
        assert!(body.contains("ready_but_idle=1"), "{body}");
        assert!(body.contains("unique_terminal=1"), "{body}"); // 2 terminal events, 1 unique lane
        assert!(body.contains("verdict=prompt_acceptance_timeout"));
        assert!(body.contains("verdict=transport_dead"));
        assert!(body.contains("verdict=ready_but_idle"));
        assert!(body.contains("verdict=ok"));
        assert!(body.contains("fire=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn lane_health_route_wired_read_only_no_fire() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/lane/health", "");
        assert_ne!(code, 404, "lane health route must be wired");
        assert!(
            body.contains("classifier=worker_lifecycle_6way") || body.contains("ledger_unreadable")
        );
        assert!(body.contains("fire=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(route(&sh, "POST", "/api/lane/health", "fire=1").0, 404);
    }
}

#[cfg(test)]
mod policy_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_pr_context_requires_id_ci_defaults_true() {
        let p = parse_pr_context(r#"{"id":"PR1","clean":true,"additive":true}"#).unwrap();
        assert_eq!(p.id, "PR1");
        assert!(p.ci_ok); // defaults true (no-CI repos don't block)
        assert!(p.clean && p.additive);
        assert!(parse_pr_context(r#"{"no_id":1}"#).is_none());
    }

    #[test]
    fn render_policy_reproduces_the_hand_triage() {
        // mirrors the real backlog: a clean additive findings PR (merge), a superseded (close),
        // a DIRTY federation PR (rebase), a sensitive root PR (hold), an author-forbids PR (hold).
        let ledger = [
            r#"{"id":"findings","clean":true,"additive":true}"#,
            r#"{"id":"old12","superseded":true,"clean":true,"additive":true}"#,
            r#"{"id":"fed7","dirty":true,"additive":true}"#,
            r#"{"id":"root4","sensitive":true,"clean":true,"additive":true}"#,
            r#"{"id":"alg4","author_forbids":true,"clean":true,"additive":true}"#,
        ]
        .join("\n");
        let (code, body) = render_policy(&ledger);
        assert_eq!(code, 200);
        assert!(body.contains("engine=pr_triage_policy"));
        assert!(body.contains("total=5"));
        assert!(body.contains("merge=1"), "{body}");
        assert!(body.contains("close=1"), "{body}");
        assert!(body.contains("rebase=1"), "{body}");
        assert!(body.contains("hold=2"), "{body}");
        assert!(body.contains("id=findings|action=merge"));
        assert!(body.contains("id=old12|action=close"));
        assert!(body.contains("id=fed7|action=rebase"));
        assert!(body.contains("id=root4|action=hold"));
        assert!(body.contains("execute=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn policy_route_wired_read_only_no_execute() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/policy/evaluate", "");
        assert_ne!(code, 404, "policy route must be wired");
        assert!(body.contains("engine=pr_triage_policy") || body.contains("ledger_unreadable"));
        assert!(body.contains("execute=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(
            route(&sh, "POST", "/api/policy/evaluate", "execute=1").0,
            404
        );
    }
}

#[cfg(test)]
mod branch_freshness_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_branch_requires_id_counts_default_zero() {
        let r = parse_branch(r#"{"id":"b1","ahead_by":2,"behind_by":5}"#).unwrap();
        assert_eq!(r.id, "b1");
        assert_eq!(r.ahead_by, Some(2));
        assert_eq!(r.behind_by, Some(5));
        assert_eq!(r.conflicting, None);
        let bare = parse_branch(r#"{"id":"b2"}"#).unwrap();
        assert_eq!(bare.ahead_by, None); // missing counts => unknown/block, never fake-fresh
        assert_eq!(bare.behind_by, None);
        assert!(parse_branch(r#"{"no_id":true}"#).is_none());
    }

    #[test]
    fn parse_branch_rejects_negative_or_fractional_counts() {
        let neg = parse_branch(r#"{"id":"b1","ahead_by":-1,"behind_by":0,"conflicting":false}"#)
            .unwrap();
        assert_eq!(neg.ahead_by, None);
        let frac = parse_branch(r#"{"id":"b2","ahead_by":1.5,"behind_by":0,"conflicting":false}"#)
            .unwrap();
        assert_eq!(frac.ahead_by, None);
    }

    #[test]
    fn render_branch_freshness_classifies_and_flags_stale_noise() {
        // up-to-date (ready), behind-no-own (merge_forward), diverged-clean (rebase),
        // diverged-conflicting (block). Only the fresh one is NOT stale-noise.
        let ledger = [
            r#"{"id":"fresh1","ahead_by":3,"behind_by":0}"#,
            r#"{"id":"stale1","ahead_by":0,"behind_by":7,"conflicting":false}"#,
            r#"{"id":"div1","ahead_by":2,"behind_by":4,"conflicting":false}"#,
            r#"{"id":"conf1","ahead_by":2,"behind_by":4,"conflicting":true}"#,
        ]
        .join("\n");
        let (code, body) = render_branch_freshness(&ledger);
        assert_eq!(code, 200);
        assert!(body.contains("detector=stale_branch_freshness"));
        assert!(body.contains("total=4"));
        assert!(body.contains("ready=1"), "{body}");
        assert!(body.contains("merge_forward=1"), "{body}");
        assert!(body.contains("rebase=1"), "{body}");
        assert!(body.contains("block=1"), "{body}");
        assert!(body.contains("stale_noise=3"), "{body}"); // all non-fresh
        assert!(body.contains("id=fresh1|freshness=fresh|action=ready|stale_noise=false"));
        assert!(body.contains("id=stale1|freshness=stale|action=merge_forward|stale_noise=true"));
        assert!(body.contains("id=div1|freshness=diverged|action=rebase|stale_noise=true"));
        assert!(body.contains("id=conf1|freshness=diverged|action=block|stale_noise=true"));
        assert!(body.contains("fire=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn render_branch_freshness_missing_fields_blocks_not_ready() {
        let ledger = [
            r#"{"id":"missing-counts"}"#,
            r#"{"id":"missing-conflict","ahead_by":0,"behind_by":3}"#,
        ]
        .join("\n");
        let (code, body) = render_branch_freshness(&ledger);
        assert_eq!(code, 200);
        assert!(body.contains("total=2"));
        assert!(body.contains("ready=0"), "{body}");
        assert!(body.contains("block=2"), "{body}");
        assert!(body.contains("id=missing-counts|freshness=unknown|action=block"));
        assert!(body.contains("id=missing-conflict|freshness=stale|action=block"));
    }

    #[test]
    fn render_branch_freshness_empty_is_clean_zero_not_faked() {
        let (code, body) = render_branch_freshness("");
        assert_eq!(code, 200);
        assert!(body.contains("total=0"));
        assert!(body.contains("ready=0"));
        assert!(body.contains("stale_noise=0"));
        assert!(body.contains("fire=false"));
    }

    #[test]
    fn branch_freshness_route_wired_read_only_no_fire() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/branch/freshness", "");
        assert_ne!(code, 404, "branch freshness route must be wired");
        assert!(
            body.contains("detector=stale_branch_freshness") || body.contains("ledger_unreadable")
        );
        assert!(body.contains("fire=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(route(&sh, "POST", "/api/branch/freshness", "fire=1").0, 404);
    }
}
