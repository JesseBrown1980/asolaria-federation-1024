//! vote-quorum-parity — std READ-ONLY parity harness.
//!
//! Proves the no_std `asolaria-server-vote-quorum` lib (`evaluate`) and `canon` (sha-chain
//! `recompute`) reproduce the LIVE python `:4952` daemon's on-disk ledgers, before any cutover.
//! This is the "additive-until-parity" verification step: it does NOT touch the python daemon,
//! does NOT bind a port, and does NOT write any ledger — it only reads and compares.
//!
//! Two checks against `C:/HyperBEHCS/data/vote-quorum/{queue,votes,outcomes}.ndjson`:
//!   1. sha-chain — every row's stored `row_hash` == `canon::recompute(row)`, and each row's
//!      `antecedents` == the previous row's `row_hash` (first row anchors to zero).
//!   2. outcome-replay — for every resolved `outcomes.ndjson` row, gather that vote_id's cast
//!      votes and confirm `evaluate(class, votes)` yields the SAME outcome the daemon recorded.
//!
//! Output is HBP tuple-text (`json=0`). Exit 0 iff full parity, else 1.

use std::fs;
use std::process::exit;

use asolaria_server_vote_quorum::canon;
use asolaria_server_vote_quorum::{evaluate, Outcome, Vote};

const DATA_DIR: &str = "C:/HyperBEHCS/data/vote-quorum";

/// Extract a flat top-level `"key": <value>` field. Handles string values (`"..."`, no embedded
/// quotes — true for vote_id/voter_pid/vote/decision_class/outcome/antecedents) and bare tokens.
fn field(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":");
    let start = line.find(&pat)? + pat.len();
    let rest = line[start..].trim_start();
    if let Some(inner) = rest.strip_prefix('"') {
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}

fn parse_vote(s: &str) -> Option<Vote> {
    match s {
        "YES" => Some(Vote::Yes),
        "NO" => Some(Vote::No),
        "ABSTAIN" => Some(Vote::Abstain),
        _ => None,
    }
}

/// Map the lib `Outcome` enum to the exact python outcome string persisted in `outcomes.ndjson`.
fn outcome_str(o: Outcome) -> &'static str {
    match o {
        Outcome::PassUnanimous => "PASS-UNANIMOUS",
        Outcome::PassSupermajority => "PASS-SUPERMAJORITY",
        Outcome::PassMajority => "PASS-MAJORITY",
        Outcome::PassAuto => "PASS-AUTO",
        Outcome::Reject => "REJECT",
        Outcome::Pending => "PENDING",
    }
}

/// Returns `(rows, present)`. `present=false` means the file could not be read (missing /
/// permission / wrong path) — which is NOT the same as an empty-but-readable ledger. The caller
/// treats an absent ledger as a FAIL, never a vacuous zero-row pass (claims-gate: missing ≠ clean-zero).
fn read_rows(path: &str) -> (Vec<String>, bool) {
    match fs::read_to_string(path) {
        Ok(s) => (
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect(),
            true,
        ),
        Err(_) => (Vec::new(), false),
    }
}

/// (hash_ok, hash_fail, link_fail) over one ledger's sha-chain.
fn verify_chain(rows: &[String]) -> (usize, usize, usize) {
    let (mut hash_ok, mut hash_fail, mut link_fail) = (0usize, 0usize, 0usize);
    let mut prev = String::from("0000000000000000");
    for line in rows {
        match (canon::recompute(line), canon::stored_row_hash(line)) {
            (Ok(rc), Some(st)) if rc == st => hash_ok += 1,
            _ => hash_fail += 1,
        }
        if let Some(ant) = field(line, "antecedents") {
            if ant != prev {
                link_fail += 1;
            }
        }
        if let Some(st) = canon::stored_row_hash(line) {
            prev = st;
        }
    }
    (hash_ok, hash_fail, link_fail)
}

fn main() {
    // Data dir overridable by argv[1] so the same binary works native-Windows (`C:/...`, default)
    // and under the WSL 1.81 CI toolchain (`/mnt/c/...`).
    let dir = std::env::args().nth(1).unwrap_or_else(|| DATA_DIR.to_string());
    let (queue, q_present) = read_rows(&format!("{dir}/queue.ndjson"));
    let (votes, v_present) = read_rows(&format!("{dir}/votes.ndjson"));
    let (outcomes, o_present) = read_rows(&format!("{dir}/outcomes.ndjson"));
    let all_present = q_present && v_present && o_present;

    let (q_ok, q_hf, q_lf) = verify_chain(&queue);
    let (v_ok, v_hf, v_lf) = verify_chain(&votes);
    let (o_ok, o_hf, o_lf) = verify_chain(&outcomes);
    let chain_fail = q_hf + q_lf + v_hf + v_lf + o_hf + o_lf;

    // Dedup outcomes to the TERMINAL row per vote_id. Appends are chronological, so the LAST row for
    // a vote_id is its resolved state; earlier rows for the same vote_id are SUPERSEDED. The py
    // daemon's `outcome_exists` idempotence check races under concurrent casts (observed in the
    // 2026-05-24 AGT-*-stress rows: 3 outcome rows for one USB_WRITE vote), so it can persist
    // intermediate outcome rows. Those are a py-daemon observation, NOT a Rust-logic mismatch —
    // parity is judged against the terminal outcome each vote actually resolved to.
    let mut terminal: Vec<(String, String)> = Vec::new(); // (vote_id, terminal_row)
    let mut superseded = 0usize;
    let mut replay_skip = 0usize;
    for orow in &outcomes {
        match field(orow, "vote_id") {
            Some(vid) => {
                if let Some(slot) = terminal.iter_mut().find(|(k, _)| *k == vid) {
                    slot.1 = orow.clone();
                    superseded += 1;
                } else {
                    terminal.push((vid, orow.clone()));
                }
            }
            None => replay_skip += 1,
        }
    }

    let (mut replay_match, mut replay_mismatch) = (0usize, 0usize);
    let mut mismatches: Vec<String> = Vec::new();
    for (vid, orow) in &terminal {
        let (class, recorded) = match (field(orow, "decision_class"), field(orow, "outcome")) {
            (Some(b), Some(c)) => (b, c),
            _ => {
                replay_skip += 1;
                continue;
            }
        };
        let owned: Vec<(String, Vote)> = votes
            .iter()
            .filter(|v| field(v, "vote_id").as_deref() == Some(vid.as_str()))
            .filter_map(|v| Some((field(v, "voter_pid")?, parse_vote(&field(v, "vote")?)?)))
            .collect();
        let refs: Vec<(&str, Vote)> = owned.iter().map(|(s, v)| (s.as_str(), *v)).collect();
        let (got, _tally) = evaluate(&class, &refs);
        if outcome_str(got) == recorded {
            replay_match += 1;
        } else {
            replay_mismatch += 1;
            if mismatches.len() < 12 {
                mismatches.push(format!(
                    "MISMATCH|vote_id={vid}|class={class}|recorded={recorded}|replayed={}|votes={}",
                    outcome_str(got),
                    owned.len()
                ));
            }
        }
    }

    // A missing ledger, or zero outcomes to replay, cannot prove parity. Fail loudly — never a
    // vacuous zero-row PASS (claims-gate scenario: missing/empty ≠ clean-zero-ok).
    let has_data = outcomes.len() + votes.len() + queue.len() > 0;
    let parity = all_present && has_data && chain_fail == 0 && replay_mismatch == 0 && replay_match > 0;

    println!("VOTEQPARITYRUN|format=hbp_tuple_text|json=0|read_only=1|touched_daemon=0|wrote_ledger=0");
    println!(
        "VOTEQPARITYSRC|dir={dir}|present={all_present}|q_present={q_present}|v_present={v_present}|o_present={o_present}|queue_rows={}|votes_rows={}|outcomes_rows={}",
        queue.len(),
        votes.len(),
        outcomes.len()
    );
    println!("VOTEQPARITYCHAIN|ledger=queue|hash_ok={q_ok}|hash_fail={q_hf}|link_fail={q_lf}");
    println!("VOTEQPARITYCHAIN|ledger=votes|hash_ok={v_ok}|hash_fail={v_hf}|link_fail={v_lf}");
    println!("VOTEQPARITYCHAIN|ledger=outcomes|hash_ok={o_ok}|hash_fail={o_hf}|link_fail={o_lf}");
    println!(
        "VOTEQPARITYREPLAY|outcome_rows={}|unique_vote_ids={}|superseded_py_idempotence_race={superseded}|match={replay_match}|mismatch={replay_mismatch}|skip={replay_skip}",
        outcomes.len(),
        terminal.len()
    );
    for m in &mismatches {
        println!("VOTEQPARITY{m}");
    }
    let verdict = if parity {
        "PASS"
    } else if !all_present || !has_data {
        "FAIL_NODATA"
    } else {
        "FAIL"
    };
    println!(
        "VOTEQPARITYVERDICT|parity={verdict}|all_present={all_present}|chain_fail={chain_fail}|replay_mismatch={replay_mismatch}|note=lib+canon vs live py :4952 ledgers; MEASURED read-only"
    );

    exit(if parity { 0 } else { 1 });
}
