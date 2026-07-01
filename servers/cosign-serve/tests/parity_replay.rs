use std::fs;
use std::path::Path;

use asolaria_server_cosign_ledger::py_parity::{canonical_bytes, parse_line, sha16};

#[derive(Debug, Default, PartialEq, Eq)]
struct ReplayStats {
    rows: u64,
    checked: u64,
    matched: u64,
    mismatches: u64,
    legacy: u64,
}

fn replay(path: &Path) -> ReplayStats {
    let mut stats = ReplayStats::default();
    let Ok(body) = fs::read_to_string(path) else {
        return stats;
    };
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        stats.rows += 1;
        let Ok(row) = parse_line(line.as_bytes()) else {
            stats.legacy += 1;
            continue;
        };
        let Some(disk) = row.row_hash() else {
            stats.legacy += 1;
            continue;
        };
        stats.checked += 1;
        let got = sha16(&canonical_bytes(&row));
        if got == disk {
            stats.matched += 1;
        } else {
            stats.mismatches += 1;
        }
    }
    stats
}

#[test]
fn fixture_replay_matches_all_row_hashes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/daemon-row-hash.ndjson");
    let stats = replay(&path);
    assert_eq!(stats.rows, 2);
    assert_eq!(stats.checked, 2);
    assert_eq!(stats.matched, 2);
    assert_eq!(stats.mismatches, 0);
}

#[test]
#[ignore = "requires an operator-provided read-only COSIGN_CHAIN.ndjson path"]
fn full_live_replay_matches_all_daemon_rows() {
    let path = std::env::var("ASOLARIA_COSIGN_NDJSON")
        .expect("set ASOLARIA_COSIGN_NDJSON to a read-only ledger copy");
    let stats = replay(Path::new(&path));
    assert!(
        stats.checked > 0,
        "ledger must include daemon row_hash rows"
    );
    assert_eq!(stats.checked, stats.matched, "{stats:?}");
    assert_eq!(stats.mismatches, 0, "{stats:?}");
}
