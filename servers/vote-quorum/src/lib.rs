//! asolaria-server-vote-quorum — Rust port of the py vote-quorum daemon (:4952): the QUORUM_RULES
//! table, evaluate_quorum, and is_authorized, plus the sha-chain canon (parity with the daemon's
//! append_row). Pure no_std (alloc + sha2). STAGED: a crate of the council/loop Host-8 responder;
//! the live py :4952 stays untouched, no cutover. The std service + parity-harness that reads the
//! on-disk ledgers lives in the council-serve bin (follow-up).

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;

pub mod canon;

/// The 5 apex operators (LAW v3 quintuple window). Matches py `QUINTUPLE`.
pub const QUINTUPLE: [&str; 5] = ["OP-JESSE", "OP-RAYSSA", "OP-AMY", "OP-DAN", "OP-FELIPE"];
/// Authorized-pool class prefixes (2-month OP window). Matches py `AUTHORIZED_CLASSES`.
pub const AUTHORIZED_CLASSES: [&str; 4] = ["Special-OP", "OP", "Asolaria-PID", "Profile"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pool {
    Quintuple,
    Authorized,
    Any,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Unanimous5,
    Supermajority23,
    SimpleMajority,
    AutoPass,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vote {
    Yes,
    No,
    Abstain,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    PassUnanimous,
    PassSupermajority,
    PassMajority,
    PassAuto,
    Reject,
    Pending,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tally {
    pub yes: u32,
    pub no: u32,
    pub abstain: u32,
    pub total_decisive: u32,
}

/// `(rule, pool)` per decision class — matches the py daemon `QUORUM_RULES` table exactly.
pub fn rule_for(class: &str) -> (Rule, Pool) {
    match class {
        "LAW_CHANGE" | "CP_MINT" => (Rule::Unanimous5, Pool::Quintuple),
        "USB_WRITE" => (Rule::Supermajority23, Pool::Authorized),
        "HEARTBEAT_ACK" => (Rule::AutoPass, Pool::Any),
        // DAEMON_OP | MEMORY_WRITE | COSIGN_APPEND | GHOST_GC | DEFAULT | unknown
        _ => (Rule::SimpleMajority, Pool::Authorized),
    }
}

/// Matches py `is_authorized`: quintuple pool = voter in QUINTUPLE; authorized = QUINTUPLE or, for
/// each class `c`, `voter.startswith("AGT-"+c)` or `voter.startswith(c)`; any = true.
pub fn is_authorized(voter: &str, pool: Pool) -> bool {
    match pool {
        Pool::Quintuple => QUINTUPLE.contains(&voter),
        Pool::Any => true,
        Pool::Authorized => {
            QUINTUPLE.contains(&voter)
                || AUTHORIZED_CLASSES.iter().any(|c| {
                    voter.starts_with(c) || (voter.starts_with("AGT-") && voter[4..].starts_with(c))
                })
        }
    }
}

/// `evaluate_quorum`: filter to authorized voters, exclude abstains from the decisive total, then
/// apply the rule. Integer forms of the daemon's float comparisons:
/// supermajority `yes >= 2*total/3` => `3*yes >= 2*total`; reject `no > total/3` => `3*no > total`;
/// simple `yes > total/2` => `2*yes > total`; reject `no >= total/2` => `2*no >= total`.
pub fn evaluate(class: &str, votes: &[(&str, Vote)]) -> (Outcome, Tally) {
    let (rule, pool) = rule_for(class);
    let cast: Vec<&(&str, Vote)> = votes
        .iter()
        .filter(|(v, _)| is_authorized(v, pool))
        .collect();
    let yes = cast.iter().filter(|(_, v)| *v == Vote::Yes).count() as u32;
    let no = cast.iter().filter(|(_, v)| *v == Vote::No).count() as u32;
    let abstain = cast.iter().filter(|(_, v)| *v == Vote::Abstain).count() as u32;
    let total = yes + no;
    let tally = Tally {
        yes,
        no,
        abstain,
        total_decisive: total,
    };
    let (y, n, tt) = (yes as u64, no as u64, total as u64);
    let outcome = match rule {
        Rule::AutoPass => Outcome::PassAuto,
        Rule::Unanimous5 => {
            let all5_yes = QUINTUPLE
                .iter()
                .all(|q| cast.iter().any(|(v, vt)| v == q && *vt == Vote::Yes));
            if all5_yes && no == 0 {
                Outcome::PassUnanimous
            } else if no > 0 {
                Outcome::Reject
            } else {
                Outcome::Pending
            }
        }
        Rule::Supermajority23 => {
            if tt >= 3 && y * 3 >= 2 * tt {
                Outcome::PassSupermajority
            } else if tt >= 3 && n * 3 > tt {
                Outcome::Reject
            } else {
                Outcome::Pending
            }
        }
        Rule::SimpleMajority => {
            if tt >= 3 && y * 2 > tt {
                Outcome::PassMajority
            } else if tt >= 3 && n * 2 >= tt {
                Outcome::Reject
            } else {
                Outcome::Pending
            }
        }
    };
    (outcome, tally)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn outc(c: &str, v: &[(&str, Vote)]) -> Outcome {
        evaluate(c, v).0
    }
    #[test]
    fn auto_pass() {
        assert_eq!(outc("HEARTBEAT_ACK", &[]), Outcome::PassAuto);
    }
    #[test]
    fn unanimous5_pass_pending_reject() {
        let all = [
            ("OP-JESSE", Vote::Yes),
            ("OP-RAYSSA", Vote::Yes),
            ("OP-AMY", Vote::Yes),
            ("OP-DAN", Vote::Yes),
            ("OP-FELIPE", Vote::Yes),
        ];
        assert_eq!(outc("LAW_CHANGE", &all), Outcome::PassUnanimous);
        let four = [
            ("OP-JESSE", Vote::Yes),
            ("OP-RAYSSA", Vote::Yes),
            ("OP-AMY", Vote::Yes),
            ("OP-DAN", Vote::Yes),
        ];
        assert_eq!(outc("LAW_CHANGE", &four), Outcome::Pending);
        let nope = [("OP-JESSE", Vote::Yes), ("OP-RAYSSA", Vote::No)];
        assert_eq!(outc("LAW_CHANGE", &nope), Outcome::Reject);
    }
    #[test]
    fn simple_majority_pass_reject_pending() {
        let pass = [
            ("OP-JESSE", Vote::Yes),
            ("OP-RAYSSA", Vote::Yes),
            ("OP-AMY", Vote::No),
        ];
        assert_eq!(outc("DAEMON_OP", &pass), Outcome::PassMajority); // total3 2*2>3
        let tie = [
            ("OP-JESSE", Vote::Yes),
            ("OP-RAYSSA", Vote::No),
            ("OP-AMY", Vote::No),
            ("OP-DAN", Vote::Yes),
        ];
        assert_eq!(outc("DAEMON_OP", &tie), Outcome::Reject); // total4 2*2>=4
        let two = [("OP-JESSE", Vote::Yes), ("OP-RAYSSA", Vote::Yes)];
        assert_eq!(outc("DAEMON_OP", &two), Outcome::Pending); // total<3
    }
    #[test]
    fn supermajority_pass_pending() {
        let pass = [
            ("OP-JESSE", Vote::Yes),
            ("OP-RAYSSA", Vote::Yes),
            ("OP-AMY", Vote::No),
        ];
        assert_eq!(outc("USB_WRITE", &pass), Outcome::PassSupermajority); // total3 yes2 3*2>=2*3
        let pend = [("OP-JESSE", Vote::Yes), ("OP-RAYSSA", Vote::Yes)];
        assert_eq!(outc("USB_WRITE", &pend), Outcome::Pending); // total<3
    }
    #[test]
    fn abstains_excluded_from_total() {
        let a = [
            ("OP-JESSE", Vote::Yes),
            ("OP-RAYSSA", Vote::Yes),
            ("OP-AMY", Vote::Abstain),
        ];
        let (_, t) = evaluate("DAEMON_OP", &a);
        assert_eq!(t.total_decisive, 2);
        assert_eq!(t.abstain, 1);
    }
    #[test]
    fn unauthorized_voter_filtered() {
        let u = [
            ("OP-JESSE", Vote::Yes),
            ("RANDO", Vote::Yes),
            ("OP-RAYSSA", Vote::Yes),
        ];
        let (_, t) = evaluate("DAEMON_OP", &u);
        assert_eq!(t.yes, 2); // RANDO not authorized -> filtered
    }
    #[test]
    fn agt_class_prefix_authorized() {
        assert!(is_authorized("AGT-Profile-x9", Pool::Authorized));
        assert!(is_authorized("OP-anything", Pool::Authorized));
        assert!(!is_authorized("RANDO", Pool::Authorized));
        assert!(!is_authorized("OP-anything", Pool::Quintuple));
    }
}
