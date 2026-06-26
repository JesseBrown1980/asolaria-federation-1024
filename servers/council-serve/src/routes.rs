//! Read-only json=0 routes. The centerpiece is `/api/vote-quorum/parity` — the on-disk vote-ledger
//! parity harness liris asked for: recompute each row's row_hash via the vote-quorum `canon` recipe
//! and compare to the stored value (proves the Rust canon == the py daemon `append_row`). The live
//! ledger files are NEVER written. No append/cutover/fire route exists in this build.

use std::fs;
use std::sync::Arc;

use asolaria_server_vote_quorum::canon;

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
            String::from("COUNCILSTATUS|ok=1|responder=live|engine=gated|cutover=false|json=0\n"),
        ),
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
