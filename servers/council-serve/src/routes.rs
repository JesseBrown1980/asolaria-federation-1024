//! Read-only json=0 routes. The centerpiece is `/api/vote-quorum/parity` — the on-disk vote-ledger
//! parity harness liris asked for: recompute each row's row_hash via the vote-quorum `canon` recipe
//! and compare to the stored value (proves the Rust canon == the py daemon `append_row`). The live
//! ledger files are NEVER written. No append/cutover/fire route exists in this build.

use std::fs;
use std::sync::Arc;

use asolaria_server_vote_quorum::canon;

use crate::lane_event;
use crate::lane_health;
use crate::mcp_health;
use crate::policy;
use crate::ptc_dispatch;
use crate::recovery;
use crate::schedule;
use crate::stale_branch;
use crate::Shared;

fn hbp_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            // '|' CR LF tab break the pipe-delimited line; '{' '}' would emit a brace into json=0
            // output (the no-brace invariant) — neutralize all of them.
            '|' | '\r' | '\n' | '\t' | '{' | '}' => '_',
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
        ("GET", "/api/mcp/health") => mcp_health_route(),
        ("GET", "/api/lane/events") => lane_events_route(),
        ("GET", "/api/lane/recovery") => recovery_route(),
        ("GET", "/api/ptc/run") => ptc_route(),
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
        env_q("ASOLARIA_ACCEPT_THRESHOLD", 0.5),
    )
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Read a q value (per-mille integer, 0..=1000) from the environment. Integer only.
fn env_q(key: &str, default_q: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
        .filter(|q| *q <= 1000)
        .unwrap_or(default_q)
}

/// Pure: parse an ndjson loop ledger, run the confidence schedule, render json=0 HBP rows. Never fires.
fn render_schedule(body: &str, budget: usize, threshold_q: u16) -> (u16, String) {
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
    let sched = schedule::confidence_schedule(&proposals, budget, threshold_q);
    let verify_n = sched.iter().filter(|s| s.verify).count();
    let mut out = format!(
        "COUNCILSCHEDULE|ok=1|status=staged|read_only=true|scheduler=confidence_scheduled_verify|total={}|verify={}|defer={}|legacy={}|budget={}|threshold_q={}|fire=false|cutover=false|json=0\n",
        proposals.len(),
        verify_n,
        proposals.len().saturating_sub(verify_n),
        legacy,
        budget,
        threshold_q
    );
    for s in &sched {
        out.push_str(&format!(
            "COUNCILSCHED|rank={}|id={}|confidence_q={}|action={}|fire=false|json=0\n",
            s.rank,
            hbp_escape(&s.id),
            s.confidence_q,
            if s.verify { "VERIFY" } else { "DEFER" }
        ));
    }
    (200, out)
}

/// Parse one ndjson loop-ledger line into a Proposal. Requires `id` + `score` (GNN confidence);
/// `genius` falls back to `score` (reverse-gain not yet run), `risk` to 0.0. Unparseable -> None.
fn parse_proposal(line: &str) -> Option<schedule::Proposal> {
    let id = json_str(line, "id")?;
    let score_q = json_unit_q(line, "score")?;
    let genius_q = json_unit_q(line, "genius").unwrap_or(score_q);
    let risk_q = json_unit_q(line, "risk").unwrap_or(0);
    Some(schedule::Proposal {
        id,
        score_q,
        genius_q,
        risk_q,
    })
}

/// Locate a JSON key at an OBJECT-KEY position: the first `"key"` occurrence immediately followed
/// (after optional whitespace) by `:`. Returns the value slice (everything after the colon, leading
/// whitespace not trimmed). This rejects a key NAME that appears inside a string VALUE — e.g. for key
/// `lane`, the row `{"note":"lane","lane":"real"}` skips the value occurrence and finds the real key.
/// (Hardening: the previous helpers used a bare substring find and could be fooled by such values.)
fn find_key<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\"");
    let mut rest = s;
    loop {
        let rel = rest.find(&pat)?;
        let after = &rest[rel + pat.len()..];
        if let Some(value) = after.trim_start().strip_prefix(':') {
            return Some(value);
        }
        rest = after; // this occurrence wasn't a key position; keep searching past it
    }
}

fn json_str(s: &str, key: &str) -> Option<String> {
    let after = find_key(s, key)?.trim_start().strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Parse a JSON number in [0, 1] into q (per-mille, 0..=1000) WITHOUT float: the digit run is
/// read directly, at most three fractional digits are used, the fourth rounds half-up.
/// Values above 1 clamp to 1000. Negative -> 0. Exponent notation is rejected (None).
fn json_unit_q(s: &str, key: &str) -> Option<u16> {
    let after = find_key(s, key)?.trim_start();
    let end = after
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(after.len());
    let tok = after.get(..end)?.trim();
    if tok.is_empty() || tok.contains('e') || tok.contains('E') {
        return None;
    }
    let neg = tok.starts_with('-');
    let tok = tok.trim_start_matches(['-', '+']);
    let (int_part, frac_part) = match tok.split_once('.') {
        Some((i, f)) => (i, f),
        None => (tok, ""),
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
    // three digits of per-mille, fourth rounds half-up
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
    Some(milli.min(1000) as u16)
}

fn json_bool(s: &str, key: &str) -> Option<bool> {
    let after = find_key(s, key)?.trim_start();
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
        elapsed_ms: json_u64(line, "elapsed_ms").unwrap_or(0),
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

/// Non-negative integer field. Uses the PRECISE `json_u64` digit-run parser, not `json_num`:
/// the former float-routed helper silently corrupted values above 2^53 (the same hazard the FNV
/// `host_handle8` path already documents). Integer only.
fn json_nonnegative_u64(s: &str, key: &str) -> Option<u64> {
    json_u64(s, key)
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

/// Read-only MCP/fabric degraded-mode route (absorbed claw-code MCP degraded-mode). Reads a lane
/// evidence ledger (ndjson) from `ASOLARIA_MCP_LEDGER`; unset/empty -> staged (never fake-clean),
/// unreadable -> 500. Classifies each lane healthy/degraded/down/unknown + names why, and upholds the
/// claims-gate invariant (a fallback/stale lane is NEVER live). CLASSIFIES ONLY — never probes/fires.
fn mcp_health_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_MCP_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("MCPHEALTH|ok=1|status=staged|read_only=true|source=mcp_ledger_unwired|hint=set_ASOLARIA_MCP_LEDGER|detector=mcp_degraded_mode|fire=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "MCPHEALTH|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_mcp_health(
        &body,
        env_usize("ASOLARIA_MCP_STALE_MS", 30_000) as u64,
        env_usize("ASOLARIA_MCP_TIMEOUT_MS", 10_000) as u64,
    )
}

/// Parse one ndjson lane-evidence row -> mcp_health::Evidence. Requires `lane`; everything else
/// defaults to the PESSIMISTIC side (probed/transport_ok/responded/response_present=false) so a bare
/// row can never reach Healthy. `age_ms` absent OR negative (future-dated) -> None. `raw` = the whole
/// line, so a buried HBPFALLBACK / *_fallback is caught even if the producer omitted the bool.
fn parse_mcp(line: &str) -> Option<mcp_health::Evidence> {
    let lane = json_str(line, "lane")?;
    let capability = json_str(line, "capability").unwrap_or_else(|| lane.clone());
    // whole milliseconds, precise digit-run parse (integer only). A negative or non-numeric
    // value fails the parse and reads as absent -> unprovable freshness, same as before.
    let age_ms = json_u64(line, "age_ms");
    Some(mcp_health::Evidence {
        lane,
        capability,
        probed: json_bool(line, "probed").unwrap_or(false),
        transport_ok: json_bool(line, "transport_ok").unwrap_or(false),
        responded: json_bool(line, "responded").unwrap_or(false),
        response_present: json_bool(line, "response_present").unwrap_or(false),
        server_error: json_str(line, "server_error").unwrap_or_default(),
        fallback_marker: json_bool(line, "fallback_marker").unwrap_or(false),
        fallback_key: json_bool(line, "fallback_key").unwrap_or(false),
        tainted_field: json_str(line, "tainted_field").unwrap_or_else(|| "-".to_string()),
        age_ms,
        elapsed_ms: json_u64(line, "elapsed_ms").unwrap_or(0),
        raw: line.to_string(),
    })
}

/// Pure: parse the MCP ledger, classify each lane, render json=0 HBP. Classifies only; never fires.
/// `live=` is printed true IFF health=healthy — the explicit anti-fake-clean flag.
fn render_mcp_health(body: &str, stale_after_ms: u64, timeout_ms: u64) -> (u16, String) {
    let mut lanes = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_mcp(t) {
            Some(e) => lanes.push(e),
            None => legacy += 1,
        }
    }
    let diags: Vec<_> = lanes
        .iter()
        .map(|e| (e, mcp_health::classify(e, stale_after_ms, timeout_ms)))
        .collect();
    let count = |h: mcp_health::LaneHealth| diags.iter().filter(|(_, d)| d.health == h).count();
    let healthy = count(mcp_health::LaneHealth::Healthy);
    let degraded = count(mcp_health::LaneHealth::Degraded);
    let down = count(mcp_health::LaneHealth::Down);
    let unknown = count(mcp_health::LaneHealth::Unknown);
    let fallback_tainted = diags
        .iter()
        .filter(|(_, d)| d.reason.is_fallback_taint())
        .count();
    let stale = diags.iter().filter(|(_, d)| d.reason.is_stale()).count();
    let worst = mcp_health::worst(&diags.iter().map(|(_, d)| d.clone()).collect::<Vec<_>>());
    let mut out = format!(
        "MCPHEALTH|ok=1|status=ok|read_only=true|detector=mcp_degraded_mode|total={}|healthy={}|degraded={}|down={}|unknown={}|not_live={}|fallback_tainted={}|stale={}|legacy={}|worst={}|stale_ms={}|timeout_ms={}|fire=false|json=0\n",
        lanes.len(),
        healthy,
        degraded,
        down,
        unknown,
        degraded + down + unknown,
        fallback_tainted,
        stale,
        legacy,
        worst.as_str(),
        stale_after_ms,
        timeout_ms
    );
    for (e, d) in &diags {
        out.push_str(&format!(
            "MCPLANE|lane={}|capability={}|health={}|reason={}|tainted_field={}|age_ms={}|live={}|fire=false|json=0\n",
            hbp_escape(&e.lane),
            hbp_escape(&e.capability),
            d.health.as_str(),
            d.reason.as_str(),
            hbp_escape(&d.tainted_field),
            e.age_ms.map(|a| a.to_string()).unwrap_or_else(|| "unknown".to_string()),
            mcp_health::is_live(d)
        ));
    }
    (200, out)
}

/// Read-only lane-event provenance/ordering route (absorbed claw-code lane-event envelope = BEHCS
/// envelope). Reads an event ledger (ndjson) from `ASOLARIA_EVENT_LEDGER`; unset/empty -> staged
/// (never fake-clean), unreadable -> 500. Reports per-lane integrity (gaps/dups/provenance/contiguous)
/// and whether the whole stream is replay-INTACT. ANALYZES ONLY — never reorders/drops/fires.
fn lane_events_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_EVENT_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("LANEEVENTS|ok=1|status=staged|read_only=true|source=event_ledger_unwired|hint=set_ASOLARIA_EVENT_LEDGER|detector=behcs_envelope_provenance|fire=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "LANEEVENTS|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_lane_events(&body)
}

/// Precise u64 parse that reads the digit run directly. This is the ONLY numeric parser here:
/// the former float-routed helper is gone, because parsing above 2^53 through a float lost
/// precision and the full-range FNV `host_handle8` needs exact bits. Absent/non-numeric -> None.
fn json_u64(s: &str, key: &str) -> Option<u64> {
    let after = find_key(s, key)?.trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    after.get(..end)?.parse::<u64>().ok()
}

/// Parse one ndjson event row -> lane_event::Event. Requires `lane`; `host_handle8` absent -> 0 (=>
/// provenance Derived, not Verified); seq/lamport absent -> 0; hash absent -> "". u64 fields use the
/// precise `json_u64` (exact digit run, integer only). Unparseable (no lane) -> None.
fn parse_event(line: &str) -> Option<lane_event::Event> {
    let lane = json_str(line, "lane")?;
    // host_handle8: absent -> 0 (Derived). But a key that is PRESENT yet unparseable (negative /
    // > u64::MAX / non-numeric) is corrupt addressing -> substitute a value that cannot equal
    // FNV(lane) so provenance reads Mismatch, never clean-Derived (adversarial-review fix).
    let host_handle8 = match json_u64(line, "host_handle8") {
        Some(h) => h,
        None if line.contains("\"host_handle8\"") => {
            lane_event::host_handle8(&lane).wrapping_add(1) // != FNV(lane) -> Mismatch
        }
        None => 0,
    };
    Some(lane_event::Event {
        lane,
        host_handle8,
        seq: json_u64(line, "seq").unwrap_or(0),
        lamport: json_u64(line, "lamport").unwrap_or(0),
        hash: json_str(line, "hash").unwrap_or_default(),
    })
}

/// Pure: parse the event ledger, analyze per-lane integrity, render json=0 HBP. `intact=` is true
/// only if EVERY lane is contiguous (no gap), dup-free, and provenance-clean — never fake-clean over a
/// gap. Analyzes only; never reorders/drops/fires.
fn render_lane_events(body: &str) -> (u16, String) {
    let mut events = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_event(t) {
            Some(e) => events.push(e),
            None => legacy += 1,
        }
    }
    let summaries = lane_event::analyze(&events);
    // gap_count can be near-u64::MAX per corrupt lane, so SATURATE the cross-lane sum (a plain .sum()
    // would overflow-panic in debug on two huge-span lanes — a residual of the gaps() DoS fix).
    let gap_events: usize = summaries
        .iter()
        .map(|l| l.gap_count)
        .fold(0usize, usize::saturating_add);
    let dup_events: usize = summaries.iter().map(|l| l.dup_count).sum();
    let conflict_events: usize = summaries.iter().map(|l| l.conflict_count).sum();
    let provenance_mismatch = summaries
        .iter()
        .filter(|l| l.provenance == lane_event::Provenance::Mismatch)
        .count();
    // intact requires NOTHING dropped (legacy==0) AND every lane clean. An empty file (legacy 0,
    // no lanes) stays legit clean-zero=intact; a corrupt all-legacy ledger (legacy>0, 0 lanes) reads
    // intact=false instead of vacuously true (adversarial-review fix).
    let intact = legacy == 0 && summaries.iter().all(|l| l.contiguous);
    // Canonical replay order (lamport,lane,seq); count events that did NOT arrive in that order —
    // i.e. how interleaved/reordered the live stream is. 0 = the ledger was already canonical.
    let order = lane_event::canonical_order(&events);
    let out_of_order = order
        .iter()
        .enumerate()
        .filter(|(k, &arrival)| *k != arrival)
        .count();
    let mut out = format!(
        "LANEEVENTS|ok=1|status=ok|read_only=true|detector=behcs_envelope_provenance|total={}|lanes={}|gap_events={}|dup_events={}|conflict_events={}|provenance_mismatch={}|intact={}|out_of_order={}|legacy={}|fire=false|json=0\n",
        events.len(),
        summaries.len(),
        gap_events,
        dup_events,
        conflict_events,
        provenance_mismatch,
        intact,
        out_of_order,
        legacy
    );
    for l in &summaries {
        out.push_str(&format!(
            "EVLANE|lane={}|handle8={:016x}|count={}|seq_min={}|seq_max={}|gaps={}|dups={}|conflicts={}|provenance={}|contiguous={}|fire=false|json=0\n",
            hbp_escape(&l.lane),
            l.handle8,
            l.count,
            l.seq_min,
            l.seq_max,
            l.gap_count,
            l.dup_count,
            l.conflict_count,
            l.provenance.as_str(),
            l.contiguous
        ));
    }
    (200, out)
}

/// Read-only recovery-recipe route (absorbed claw-code recovery recipes + bounded-retry ledger).
/// Reads a recovery ledger (ndjson) from `ASOLARIA_RECOVERY_LEDGER`; unset/empty -> staged (never
/// fake-clean), unreadable -> 500. Decides each lane's recovery action (recipe within retry budget,
/// else escalate). DECIDES ONLY — never restarts/reconnects/re-sends/fires.
fn recovery_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_RECOVERY_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("RECOVERY|ok=1|status=staged|read_only=true|source=recovery_ledger_unwired|hint=set_ASOLARIA_RECOVERY_LEDGER|engine=recovery_recipes|execute=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "RECOVERY|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_recovery(
        &body,
        env_usize("ASOLARIA_RECOVERY_MAX_ATTEMPTS", 3).min(u32::MAX as usize) as u32,
    )
}

/// One recovery-ledger row: a lane, its classified failure, and how many attempts already made.
struct RecoveryRow {
    lane: String,
    failure: lane_health::StartupFailure,
    attempts: u32,
}

/// Parse one ndjson recovery row. Requires `lane` AND a recognized `failure` (an unknown failure
/// string is unparseable -> legacy, never silently treated as recoverable). `attempts` absent -> 0.
fn parse_recovery(line: &str) -> Option<RecoveryRow> {
    let lane = json_str(line, "lane")?;
    let failure = recovery::parse_failure(&json_str(line, "failure")?)?;
    // attempts: genuinely absent -> 0 (fresh). But a PRESENT-but-unparseable value (> u64::MAX /
    // string-quoted / negative) fails CLOSED -> u32::MAX so the lane escalates, never collapsing to
    // 0=fresh-and-recoverable (same guard as parse_event's host_handle8; adversarial-review fix).
    let attempts = match json_u64(line, "attempts") {
        Some(n) => n.min(u32::MAX as u64) as u32,
        None if line.contains("\"attempts\"") => u32::MAX,
        None => 0,
    };
    Some(RecoveryRow {
        lane,
        failure,
        attempts,
    })
}

/// Pure: parse the recovery ledger, decide each lane's recovery action, render json=0 HBP. Decides
/// only; never executes. A lane past its retry budget (and any `unknown` failure) reads `escalate`.
fn render_recovery(body: &str, max_attempts: u32) -> (u16, String) {
    let mut rows = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_recovery(t) {
            Some(r) => rows.push(r),
            None => legacy += 1,
        }
    }
    let decided: Vec<_> = rows
        .iter()
        .map(|r| (r, recovery::recommend(r.failure, r.attempts, max_attempts)))
        .collect();
    let escalate = decided
        .iter()
        .filter(|(_, (a, _))| a.is_escalation())
        .count();
    let recoverable = decided.len() - escalate;
    let exhausted = rows.iter().filter(|r| r.attempts >= max_attempts).count();
    let mut out = format!(
        "RECOVERY|ok=1|status=ok|read_only=true|engine=recovery_recipes|total={}|recoverable={}|escalate={}|exhausted={}|max_attempts={}|legacy={}|execute=false|json=0\n",
        rows.len(),
        recoverable,
        escalate,
        exhausted,
        max_attempts,
        legacy
    );
    for (r, (action, reason)) in &decided {
        out.push_str(&format!(
            "RECLANE|lane={}|failure={}|attempts={}|action={}|reason={}|execute=false|json=0\n",
            hbp_escape(&r.lane),
            r.failure.as_str(),
            r.attempts,
            action.as_str(),
            hbp_escape(reason)
        ));
    }
    (200, out)
}

/// Read-only Programmatic Tool Calling planner. Reads a declarative step ledger from
/// `ASOLARIA_PTC_LEDGER`; unset/empty -> staged. Validates the allow-listed chain and reports how
/// many tool turns/context bytes would be collapsed. PLANS ONLY — never calls tools or executes code.
fn ptc_route() -> (u16, String) {
    let path = std::env::var("ASOLARIA_PTC_LEDGER")
        .ok()
        .filter(|p| !p.is_empty());
    let Some(path) = path else {
        return (
            200,
            String::from("PTC|ok=1|status=staged|read_only=true|source=ptc_ledger_unwired|hint=set_ASOLARIA_PTC_LEDGER|engine=programmatic_tool_calling|execute=false|json=0\n"),
        );
    };
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            return (
                500,
                format!(
                    "PTC|ok=0|error=ledger_unreadable|kind={}|json=0\n",
                    hbp_escape(&e.kind().to_string())
                ),
            )
        }
    };
    render_ptc(&body)
}

/// Parse one ndjson PTC step. Requires `id` and a known allow-listed `op`; unknown op -> legacy
/// rather than recoverable. `from` is optional. `bytes` absent -> 0.
fn parse_ptc_step(line: &str) -> Option<ptc_dispatch::Step> {
    let id = json_str(line, "id")?;
    let op = ptc_dispatch::ToolOp::parse(&json_str(line, "op")?)?;
    let from = match json_str(line, "from") {
        Some(v) => Some(v),
        None if find_key(line, "from").is_some() => return None,
        None => None,
    };
    let bytes = match json_u64(line, "bytes") {
        Some(n) => n.min(usize::MAX as u64) as usize,
        None if find_key(line, "bytes").is_some() => return None,
        None => 0,
    };
    Some(ptc_dispatch::Step {
        id,
        op,
        from,
        bytes,
    })
}

/// Pure: parse the PTC ledger, validate the chain, render json=0 HBP. No execution.
fn render_ptc(body: &str) -> (u16, String) {
    let mut steps = Vec::new();
    let mut legacy = 0u64;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        match parse_ptc_step(t) {
            Some(s) => steps.push(s),
            None => legacy += 1,
        }
    }
    let p = ptc_dispatch::plan(&steps);
    let executable = p.status == ptc_dispatch::PlanStatus::Ready && legacy == 0;
    let mut out = format!(
        "PTC|ok=1|status={}|reason={}|read_only=true|engine=programmatic_tool_calling|steps={}|legacy={}|raw_turns={}|ptc_turns={}|saved_turns={}|total_bytes={}|final_bytes={}|saved_context_bytes={}|final_step={}|executable={}|execute=false|json=0\n",
        p.status.as_str(),
        p.status.reason(),
        p.steps,
        legacy,
        p.raw_turns,
        p.ptc_turns,
        p.saved_turns,
        p.total_bytes,
        p.final_bytes,
        p.saved_context_bytes,
        hbp_escape(&p.final_step),
        executable
    );
    for (idx, s) in steps.iter().enumerate() {
        out.push_str(&format!(
            "PTCSTEP|idx={}|id={}|op={}|from={}|bytes={}|execute=false|json=0\n",
            idx,
            hbp_escape(&s.id),
            s.op.as_str(),
            hbp_escape(s.from.as_deref().unwrap_or("-")),
            s.bytes
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
    fn json_key_matcher_ignores_key_name_inside_a_value() {
        // HARDENING: a key NAME appearing inside a string VALUE must not be mis-parsed as the key —
        // find_key requires the match to sit at an object-key position (followed by ':').
        assert_eq!(
            json_u64(
                r#"{"hash":"host_handle8","host_handle8":12345}"#,
                "host_handle8"
            ),
            Some(12345)
        );
        assert_eq!(
            json_str(r#"{"note":"lane","lane":"real"}"#, "lane").as_deref(),
            Some("real")
        );
        assert_eq!(
            json_bool(r#"{"x":"probed","probed":true}"#, "probed"),
            Some(true)
        );
        assert_eq!(
            json_unit_q(r#"{"label":"score","score":0.5}"#, "score"),
            Some(500)
        );
        // a key that only ever appears as a value (never a real key) -> None, not the value text
        assert_eq!(json_u64(r#"{"hash":"seq"}"#, "seq"), None);
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
        let neg =
            parse_branch(r#"{"id":"b1","ahead_by":-1,"behind_by":0,"conflicting":false}"#).unwrap();
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

#[cfg(test)]
mod mcp_health_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_mcp_requires_lane_defaults_pessimistic() {
        let e = parse_mcp(r#"{"lane":"asolaria-fabric","probed":true}"#).unwrap();
        assert_eq!(e.lane, "asolaria-fabric");
        assert_eq!(e.capability, "asolaria-fabric"); // defaults to lane
        assert!(e.probed);
        assert!(!e.transport_ok); // pessimistic default
        assert!(!e.responded);
        assert!(e.age_ms.is_none()); // absent -> None
        assert!(parse_mcp(r#"{"no_lane":true}"#).is_none());
    }

    #[test]
    fn parse_mcp_future_dated_age_is_none() {
        let e = parse_mcp(r#"{"lane":"x","age_ms":-5}"#).unwrap();
        assert!(e.age_ms.is_none()); // negative/future-dated -> unprovable
        let e2 = parse_mcp(r#"{"lane":"x","age_ms":1200}"#).unwrap();
        assert_eq!(e2.age_ms, Some(1200));
    }

    #[test]
    fn render_mcp_health_classifies_and_upholds_anti_fake_clean() {
        // 1 healthy, 1 fresh-fallback (degraded, NOT live), 1 stale (degraded), 1 transport-down
        // (down), 1 not-probed (unknown). The fallback lane must read live=false despite being fresh.
        let ledger = [
            r#"{"lane":"fab","capability":"council_query","probed":true,"transport_ok":true,"responded":true,"response_present":true,"age_ms":1000}"#,
            r#"{"lane":"sup","capability":"supervisors","probed":true,"transport_ok":true,"responded":true,"response_present":true,"age_ms":5,"fallback_marker":true}"#,
            r#"{"lane":"canon","capability":"canon_index","probed":true,"transport_ok":true,"responded":true,"response_present":true,"age_ms":999999}"#,
            r#"{"lane":"bus","capability":"bus_4944","probed":true,"transport_ok":false,"responded":false}"#,
            r#"{"lane":"recall","capability":"recall_search"}"#,
        ]
        .join("\n");
        let (code, body) = render_mcp_health(&ledger, 30_000, 10_000);
        assert_eq!(code, 200);
        assert!(body.contains("detector=mcp_degraded_mode"));
        assert!(body.contains("total=5"));
        assert!(body.contains("healthy=1"), "{body}");
        assert!(body.contains("degraded=2"), "{body}");
        assert!(body.contains("down=1"), "{body}");
        assert!(body.contains("unknown=1"), "{body}");
        assert!(body.contains("not_live=4"), "{body}");
        assert!(body.contains("fallback_tainted=1"), "{body}");
        assert!(body.contains("worst=down"), "{body}");
        // the cardinal property: fresh fallback lane is NOT live
        assert!(body.contains("lane=sup|capability=supervisors|health=degraded|reason=fallback_envelope|tainted_field=-|age_ms=5|live=false"));
        assert!(body.contains("lane=fab|capability=council_query|health=healthy|reason=none|tainted_field=-|age_ms=1000|live=true"));
        assert!(body.contains("fire=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn render_mcp_health_brace_in_field_is_escaped_no_brace_leaks() {
        // ADVERSARIAL-REVIEW FIX: a brace-bearing lane/capability must not emit a literal '{'/'}'.
        let ledger = r#"{"lane":"x{y}z","capability":"c{d}","probed":true,"transport_ok":true,"responded":true,"response_present":true,"age_ms":100}"#;
        let (code, body) = render_mcp_health(ledger, 30_000, 10_000);
        assert_eq!(code, 200);
        assert!(!body.contains('{'), "{body}");
        assert!(!body.contains('}'), "{body}");
        assert!(body.contains("lane=x_y_z")); // braces neutralized to '_'
    }

    #[test]
    fn render_mcp_health_empty_is_clean_zero_not_faked() {
        let (code, body) = render_mcp_health("", 30_000, 10_000);
        assert_eq!(code, 200);
        assert!(body.contains("total=0"));
        assert!(body.contains("healthy=0"));
        assert!(body.contains("not_live=0"));
        assert!(body.contains("worst=healthy"));
    }

    #[test]
    fn render_mcp_health_legacy_row_counted_not_dropped() {
        let ledger = "{\"lane\":\"x\",\"probed\":true,\"responded\":true,\"response_present\":true,\"transport_ok\":true,\"age_ms\":1}\n{\"no_lane\":1}\n";
        let (_, body) = render_mcp_health(ledger, 30_000, 10_000);
        assert!(body.contains("total=1"));
        assert!(body.contains("legacy=1"));
    }

    #[test]
    fn mcp_health_route_wired_read_only_no_fire() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/mcp/health", "");
        assert_ne!(code, 404, "mcp health route must be wired");
        assert!(body.contains("detector=mcp_degraded_mode") || body.contains("ledger_unreadable"));
        assert!(body.contains("fire=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(route(&sh, "POST", "/api/mcp/health", "fire=1").0, 404);
    }
}

#[cfg(test)]
mod lane_events_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn json_u64_is_precise_above_2_53() {
        // FNV handles exceed 2^53; float-routed parsing would corrupt them — json_u64 is exact.
        let big = 0xcbf2_9ce4_8422_2325u64; // 14695981039346656037
        let line = format!(r#"{{"host_handle8":{big}}}"#);
        assert_eq!(json_u64(&line, "host_handle8"), Some(big));
        assert_eq!(json_u64(r#"{"seq":42}"#, "seq"), Some(42));
        assert!(json_u64(r#"{"x":1}"#, "seq").is_none());
    }

    #[test]
    fn parse_event_requires_lane_defaults_safe() {
        let e = parse_event(r#"{"lane":"fab","seq":3,"lamport":7}"#).unwrap();
        assert_eq!(e.lane, "fab");
        assert_eq!(e.seq, 3);
        assert_eq!(e.lamport, 7);
        assert_eq!(e.host_handle8, 0); // absent -> 0 -> Derived provenance
        assert!(parse_event(r#"{"no_lane":1}"#).is_none());
    }

    #[test]
    fn render_lane_events_flags_gaps_dups_mismatch_not_fake_clean() {
        // lane A clean+verified; lane B has a gap (lost seq 2); lane C has a forged handle.
        let ha = lane_event::host_handle8("A");
        let ledger = [
            format!(r#"{{"lane":"A","host_handle8":{ha},"seq":1,"lamport":1,"hash":"a1"}}"#),
            format!(r#"{{"lane":"A","host_handle8":{ha},"seq":2,"lamport":2,"hash":"a2"}}"#),
            r#"{"lane":"B","seq":1,"lamport":1,"hash":"b1"}"#.to_string(),
            r#"{"lane":"B","seq":3,"lamport":3,"hash":"b3"}"#.to_string(), // seq 2 lost -> gap
            r#"{"lane":"C","host_handle8":48879,"seq":1,"lamport":1,"hash":"c1"}"#.to_string(), // bad handle
        ]
        .join("\n");
        let (code, body) = render_lane_events(&ledger);
        assert_eq!(code, 200);
        assert!(body.contains("detector=behcs_envelope_provenance"));
        assert!(body.contains("total=5"));
        assert!(body.contains("lanes=3"));
        assert!(body.contains("gap_events=1"), "{body}"); // B missing seq 2
        assert!(body.contains("provenance_mismatch=1"), "{body}"); // C forged
        assert!(body.contains("intact=false"), "{body}"); // gap or mismatch -> not intact
        assert!(body.contains(&format!("lane=A|handle8={ha:016x}|count=2|seq_min=1|seq_max=2|gaps=0|dups=0|conflicts=0|provenance=verified|contiguous=true")));
        assert!(
            body.contains("lane=B|")
                && body.contains("gaps=1|dups=0|conflicts=0|provenance=derived|contiguous=false")
        );
        assert!(body.contains("lane=C|") && body.contains("provenance=mismatch|contiguous=false"));
        assert!(body.contains("fire=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn render_lane_events_conflict_distinct_hash_at_same_seq() {
        // same (lane,seq) with DIFFERENT hashes = a forked slot — conflict, worse than a replay.
        let ledger = [
            r#"{"lane":"x","seq":1,"lamport":1,"hash":"v1"}"#,
            r#"{"lane":"x","seq":1,"lamport":1,"hash":"v2"}"#,
        ]
        .join("\n");
        let (_, body) = render_lane_events(&ledger);
        assert!(body.contains("dup_events=1"), "{body}");
        assert!(body.contains("conflict_events=1"), "{body}");
        assert!(body.contains("intact=false"), "{body}");
        assert!(body.contains("dups=1|conflicts=1|"), "{body}");
    }

    #[test]
    fn render_lane_events_dup_breaks_intact() {
        let ledger = "{\"lane\":\"x\",\"seq\":1}\n{\"lane\":\"x\",\"seq\":1}\n"; // same (lane,seq) twice
        let (_, body) = render_lane_events(ledger);
        assert!(body.contains("dup_events=1"), "{body}");
        assert!(body.contains("intact=false"));
    }

    #[test]
    fn render_lane_events_counts_out_of_order_arrivals() {
        // arrival order is lamport 3 then 1 then 2 -> not canonical; canonical is 1,2,3.
        let ledger = [
            r#"{"lane":"a","seq":3,"lamport":3}"#,
            r#"{"lane":"a","seq":1,"lamport":1}"#,
            r#"{"lane":"a","seq":2,"lamport":2}"#,
        ]
        .join("\n");
        let (_, body) = render_lane_events(&ledger);
        // canonical = lam1,lam2,lam3 = arrival [1,2,0]; all three positions differ from arrival.
        assert!(body.contains("out_of_order=3"), "{body}");
        // already-canonical arrival -> 0
        let ordered = [
            r#"{"lane":"a","seq":1,"lamport":1}"#,
            r#"{"lane":"a","seq":2,"lamport":2}"#,
        ]
        .join("\n");
        let (_, b2) = render_lane_events(&ordered);
        assert!(b2.contains("out_of_order=0"), "{b2}");
    }

    #[test]
    fn render_lane_events_empty_is_clean_zero_intact() {
        let (code, body) = render_lane_events("");
        assert_eq!(code, 200);
        assert!(body.contains("total=0"));
        assert!(body.contains("lanes=0"));
        assert!(body.contains("intact=true")); // empty file (legacy 0) = legit clean-zero
    }

    #[test]
    fn render_lane_events_all_legacy_is_not_intact() {
        // ADVERSARIAL-REVIEW FIX: a non-empty ledger whose every row fails parse is a BROKEN ledger,
        // not a clean one — it must NOT read intact=true vacuously.
        let ledger = "{\"node\":\"x\",\"seq\":1}\n{\"node\":\"y\",\"seq\":2}\n";
        let (_, body) = render_lane_events(ledger);
        assert!(body.contains("total=0"));
        assert!(body.contains("legacy=2"));
        assert!(body.contains("intact=false"), "{body}"); // dropped rows -> not intact
    }

    #[test]
    fn render_lane_events_present_but_invalid_handle_is_mismatch_not_clean() {
        // ADVERSARIAL-REVIEW FIX: host_handle8 present but unparseable = corrupt addressing -> the
        // lane must read provenance=mismatch / not intact, never clean-Derived.
        let ledger = r#"{"lane":"x","host_handle8":"garbage","seq":1,"lamport":1,"hash":"h"}"#;
        let (_, body) = render_lane_events(ledger);
        assert!(body.contains("provenance_mismatch=1"), "{body}");
        assert!(body.contains("provenance=mismatch"), "{body}");
        assert!(body.contains("intact=false"), "{body}");
    }

    #[test]
    fn render_lane_events_legacy_row_counted_not_dropped() {
        let ledger = "{\"lane\":\"x\",\"seq\":1}\n{\"no_lane\":1}\n";
        let (_, body) = render_lane_events(ledger);
        assert!(body.contains("total=1"));
        assert!(body.contains("legacy=1"));
    }

    #[test]
    fn lane_events_route_wired_read_only_no_fire() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/lane/events", "");
        assert_ne!(code, 404, "lane events route must be wired");
        assert!(
            body.contains("detector=behcs_envelope_provenance")
                || body.contains("ledger_unreadable")
        );
        assert!(body.contains("fire=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(route(&sh, "POST", "/api/lane/events", "fire=1").0, 404);
    }
}

#[cfg(test)]
mod recovery_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_recovery_requires_lane_and_known_failure() {
        let r = parse_recovery(r#"{"lane":"L1","failure":"worker_crashed","attempts":2}"#).unwrap();
        assert_eq!(r.lane, "L1");
        assert_eq!(r.failure, lane_health::StartupFailure::WorkerCrashed);
        assert_eq!(r.attempts, 2);
        assert!(parse_recovery(r#"{"failure":"worker_crashed"}"#).is_none()); // no lane
        assert!(parse_recovery(r#"{"lane":"x","failure":"bogus"}"#).is_none()); // unknown failure -> legacy
                                                                                // attempts absent -> 0
        assert_eq!(
            parse_recovery(r#"{"lane":"x","failure":"transport_dead"}"#)
                .unwrap()
                .attempts,
            0
        );
    }

    #[test]
    fn render_recovery_recipes_and_escalates_on_exhaustion() {
        // within budget -> recipe action; over budget -> escalate; unknown -> escalate.
        let ledger = [
            r#"{"lane":"crash1","failure":"worker_crashed","attempts":0}"#,
            r#"{"lane":"trust1","failure":"trust_required","attempts":1}"#,
            r#"{"lane":"dead1","failure":"transport_dead","attempts":5}"#, // exhausted -> escalate
            r#"{"lane":"huh1","failure":"unknown","attempts":0}"#,         // unknown -> escalate
        ]
        .join("\n");
        let (code, body) = render_recovery(&ledger, 3);
        assert_eq!(code, 200);
        assert!(body.contains("engine=recovery_recipes"));
        assert!(body.contains("total=4"));
        assert!(body.contains("recoverable=2"), "{body}"); // crash1, trust1
        assert!(body.contains("escalate=2"), "{body}"); // dead1 (exhausted), huh1 (unknown)
        assert!(body.contains("exhausted=1"), "{body}"); // dead1 attempts>=3
        assert!(
            body.contains("lane=crash1|failure=worker_crashed|attempts=0|action=restart_worker")
        );
        assert!(body.contains("lane=trust1|failure=trust_required|attempts=1|action=reissue_trust"));
        assert!(body.contains("lane=dead1|failure=transport_dead|attempts=5|action=escalate"));
        assert!(body.contains("lane=huh1|failure=unknown|attempts=0|action=escalate"));
        assert!(body.contains("execute=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn render_recovery_present_but_corrupt_attempts_escalates() {
        // ADVERSARIAL-REVIEW FIX: a present-but-unparseable attempts (> u64::MAX / string-quoted /
        // negative) must fail closed -> escalate/exhausted, never collapse to 0=fresh-recoverable.
        let ledger = [
            r#"{"lane":"a","failure":"worker_crashed","attempts":99999999999999999999}"#,
            r#"{"lane":"b","failure":"transport_dead","attempts":"7"}"#,
        ]
        .join("\n");
        let (_, body) = render_recovery(&ledger, 3);
        assert!(body.contains("escalate=2"), "{body}");
        assert!(body.contains("exhausted=2"), "{body}");
        assert!(body.contains("recoverable=0"), "{body}");
        assert!(body.contains("lane=a|failure=worker_crashed|attempts=4294967295|action=escalate"));
    }

    #[test]
    fn render_recovery_empty_is_clean_zero() {
        let (code, body) = render_recovery("", 3);
        assert_eq!(code, 200);
        assert!(body.contains("total=0"));
        assert!(body.contains("recoverable=0"));
        assert!(body.contains("escalate=0"));
    }

    #[test]
    fn render_recovery_legacy_row_counted_not_dropped() {
        let ledger = "{\"lane\":\"x\",\"failure\":\"worker_crashed\"}\n{\"lane\":\"y\",\"failure\":\"bogus\"}\n";
        let (_, body) = render_recovery(ledger, 3);
        assert!(body.contains("total=1"));
        assert!(body.contains("legacy=1")); // bogus failure -> legacy, not silently recoverable
    }

    #[test]
    fn recovery_route_wired_read_only_no_execute() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/lane/recovery", "");
        assert_ne!(code, 404, "recovery route must be wired");
        assert!(body.contains("engine=recovery_recipes") || body.contains("ledger_unreadable"));
        assert!(body.contains("execute=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(route(&sh, "POST", "/api/lane/recovery", "execute=1").0, 404);
    }
}

#[cfg(test)]
mod ptc_route_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_ptc_step_requires_id_and_allowlisted_op() {
        let s = parse_ptc_step(r#"{"id":"a","op":"recall_search","from":"seed","bytes":2048}"#)
            .unwrap();
        assert_eq!(s.id, "a");
        assert_eq!(s.op, ptc_dispatch::ToolOp::RecallSearch);
        assert_eq!(s.from.as_deref(), Some("seed"));
        assert_eq!(s.bytes, 2048);
        assert!(parse_ptc_step(r#"{"op":"recall_search"}"#).is_none());
        assert!(parse_ptc_step(r#"{"id":"x","op":"shell"}"#).is_none());
    }

    #[test]
    fn render_ptc_ready_chain_reports_turn_and_context_savings() {
        let ledger = [
            r#"{"id":"recall","op":"recall_search","bytes":4096}"#,
            r#"{"id":"health","op":"mcp_health","from":"recall","bytes":1024}"#,
            r#"{"id":"final","op":"render_hbp","from":"health","bytes":256}"#,
        ]
        .join("\n");
        let (code, body) = render_ptc(&ledger);
        assert_eq!(code, 200);
        assert!(body.contains("engine=programmatic_tool_calling"));
        assert!(body.contains("status=ready"));
        assert!(body.contains("steps=3"));
        assert!(body.contains("raw_turns=3"));
        assert!(body.contains("ptc_turns=1"));
        assert!(body.contains("saved_turns=2"));
        assert!(body.contains("total_bytes=5376"));
        assert!(body.contains("final_bytes=256"));
        assert!(body.contains("saved_context_bytes=5120"));
        assert!(body.contains("executable=true"));
        assert!(body.contains("execute=false"));
        assert!(!body.contains('{'));
    }

    #[test]
    fn render_ptc_legacy_row_prevents_executable_true() {
        let ledger = "{\"id\":\"a\",\"op\":\"recall_search\"}\n{\"id\":\"bad\",\"op\":\"shell\"}\n";
        let (_, body) = render_ptc(ledger);
        assert!(body.contains("status=ready"), "{body}"); // valid rows are plannable
        assert!(body.contains("legacy=1"), "{body}");
        assert!(body.contains("executable=false"), "{body}"); // but dropped rows block execution
    }

    #[test]
    fn render_ptc_present_but_malformed_fields_block_execution() {
        // FAIL-CLOSED: present-but-unparseable metadata must not collapse to absent/clean defaults.
        let ledger = [
            r#"{"id":"good","op":"recall_search","bytes":10}"#,
            r#"{"id":"bad_bytes","op":"mcp_health","bytes":"large"}"#,
            r#"{"id":"bad_from","op":"render_hbp","from":99}"#,
        ]
        .join("\n");
        let (_, body) = render_ptc(&ledger);
        assert!(body.contains("steps=1"), "{body}");
        assert!(body.contains("legacy=2"), "{body}");
        assert!(body.contains("executable=false"), "{body}");
    }

    #[test]
    fn render_ptc_bad_dependency_invalid_not_executable() {
        let ledger = [
            r#"{"id":"a","op":"summarize","from":"future","bytes":1}"#,
            r#"{"id":"future","op":"render_hbp","bytes":1}"#,
        ]
        .join("\n");
        let (_, body) = render_ptc(&ledger);
        assert!(body.contains("status=invalid"));
        assert!(body.contains("reason=missing_or_future_ref"));
        assert!(body.contains("executable=false"));
        assert!(body.contains("execute=false"));
    }

    #[test]
    fn render_ptc_empty_is_staged_zero_not_ready() {
        let (_, body) = render_ptc("");
        assert!(body.contains("status=empty"));
        assert!(body.contains("steps=0"));
        assert!(body.contains("executable=false"));
    }

    #[test]
    fn ptc_route_wired_read_only_no_execute() {
        let sh = Arc::new(Shared {
            vote_dir: PathBuf::from("."),
        });
        let (code, body) = route(&sh, "GET", "/api/ptc/run", "");
        assert_ne!(code, 404, "PTC route must be wired");
        assert!(
            body.contains("engine=programmatic_tool_calling") || body.contains("ledger_unreadable")
        );
        assert!(body.contains("execute=false") || body.contains("ledger_unreadable"));
        assert!(!body.contains('{'));
        assert_eq!(route(&sh, "POST", "/api/ptc/run", "execute=1").0, 404);
    }
}
