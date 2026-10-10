//! hookwall-edge — AoT Rust port of the eight Claude Code lifecycle hooks that only
//! PID-stamped their event (PostToolUse, Stop, SubagentStop, Notification,
//! UserPromptSubmit, SessionEnd, PreCompact, PermissionRequest).
//!
//! Usage: `hookwall-edge <EventName>` (the hook event is read from stdin).
//!
//! The node hooks hashed stdin, minted `cp = 256 + (sha[0..2] % 768)` and POSTed a JSON
//! envelope by curl to the bus. This port keeps the stamp exactly (same sha16, same cp) and
//! writes it hot-path: one HBP tuple row (`json=0`) appended to a local journal. No node,
//! no json, no http. stdin is hashed as a stream and never stored. Nothing is printed
//! (UserPromptSubmit would inject stdout into context). Exit 0 always: a hook never blocks.
//!
//! Journal: `$HOOKWALL_EDGE_JOURNAL`, else the FABLE5 office `hookwall-edges.hbp`. At
//! [`ROTATE_BYTES`] it is renamed to the first free `hookwall-edges.<n>.hbp` (addressed,
//! never deleted) and a fresh journal starts.

use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_JOURNAL: &str =
    "D:/PID-Registration-Office/offices/FABLE5-8467a937cba309f7/hookwall-edges.hbp";

/// Rotate the journal once it reaches 8 MiB.
const ROTATE_BYTES: u64 = 8 << 20;

/// Same mint as the node hooks: sha256 of the event bytes (`{}` when stdin was empty).
struct Stamp {
    sha16: String,
    cp: u16,
    bytes: u64,
}

fn stamp<R: Read>(mut input: R) -> Stamp {
    let mut h = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut bytes: u64 = 0;
    loop {
        match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                h.update(&buf[..n]);
                bytes += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    if bytes == 0 {
        h.update(b"{}");
    }
    let d = h.finalize();
    let mut sha16 = String::with_capacity(16);
    for b in &d[..8] {
        sha16.push_str(&format!("{b:02x}"));
    }
    let cp = 256 + (u16::from_be_bytes([d[0], d[1]]) % 768);
    Stamp { sha16, cp, bytes }
}

/// Event names are ASCII letters only; anything else is recorded as `Unknown`.
fn event_name(arg: Option<String>) -> String {
    match arg {
        Some(a) if !a.is_empty() && a.len() <= 32 && a.bytes().all(|b| b.is_ascii_alphabetic()) => {
            a
        }
        _ => "Unknown".to_string(),
    }
}

fn row(event: &str, s: &Stamp, unix_ms: u128) -> String {
    format!(
        "HOOKEDGE|event={event}|cp={}|payload_sha16={}|bytes={}|unix_ms={unix_ms}|schema=FALCON-CORRECTION-CP-MINT|from=@hookwall|json=0\n",
        s.cp, s.sha16, s.bytes
    )
}

/// First free `<stem>.<n>.<ext>` beside the journal.
fn rotated_name(journal: &Path) -> Option<PathBuf> {
    let stem = journal.file_stem()?.to_str()?;
    let ext = journal
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("hbp");
    (1u32..100_000)
        .map(|n| journal.with_file_name(format!("{stem}.{n}.{ext}")))
        .find(|p| !p.exists())
}

fn append(journal: &Path, line: &str) -> io::Result<()> {
    if fs::metadata(journal).map(|m| m.len()).unwrap_or(0) >= ROTATE_BYTES {
        if let Some(to) = rotated_name(journal) {
            let _ = fs::rename(journal, to);
        }
    }
    let mut f = OpenOptions::new().create(true).append(true).open(journal)?;
    f.write_all(line.as_bytes())
}

fn main() {
    let event = event_name(std::env::args().nth(1));
    let s = stamp(io::stdin().lock());
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let journal = std::env::var_os("HOOKWALL_EDGE_JOURNAL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_JOURNAL));
    let _ = append(&journal, &row(&event, &s, unix_ms));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stdin_matches_node_mint_of_braces() {
        // node: createHash('sha256').update('{}') = 44136fa355b3678a...
        let s = stamp(&b""[..]);
        assert_eq!(s.sha16, "44136fa355b3678a");
        assert_eq!(s.cp, 256 + (0x4413 % 768));
        assert_eq!(s.bytes, 0);
    }

    #[test]
    fn streamed_hash_equals_one_shot_hash() {
        let data = vec![b'x'; 20_000];
        let s = stamp(&data[..]);
        let d = Sha256::digest(&data);
        let mut want = String::new();
        for b in &d[..8] {
            want.push_str(&format!("{b:02x}"));
        }
        assert_eq!(s.sha16, want);
        assert_eq!(s.bytes, 20_000);
        assert!((256..1024).contains(&s.cp));
    }

    #[test]
    fn event_names_are_letters_only() {
        assert_eq!(event_name(Some("PostToolUse".into())), "PostToolUse");
        assert_eq!(event_name(Some("a|b".into())), "Unknown");
        assert_eq!(event_name(Some(String::new())), "Unknown");
        assert_eq!(event_name(None), "Unknown");
    }

    #[test]
    fn row_is_one_hbp_line_json0() {
        let s = stamp(&b"{}"[..]);
        let r = row("Stop", &s, 7);
        assert!(r.starts_with("HOOKEDGE|event=Stop|cp="));
        assert!(r.ends_with("|json=0\n"));
        assert_eq!(r.matches('\n').count(), 1);
    }

    #[test]
    fn rotation_addresses_never_deletes() {
        let dir = std::env::temp_dir().join(format!("hookwall-edge-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let j = dir.join("edges.hbp");
        fs::write(&j, vec![b'.'; ROTATE_BYTES as usize]).unwrap();
        append(&j, "ROW|json=0\n").unwrap();
        assert_eq!(fs::read_to_string(&j).unwrap(), "ROW|json=0\n");
        assert_eq!(
            fs::metadata(dir.join("edges.1.hbp")).unwrap().len(),
            ROTATE_BYTES
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
