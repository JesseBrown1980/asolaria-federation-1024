use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use asolaria_server_cosign_ledger::py_parity::{fold_seq_max, parse_line, ResumeState};

pub fn resume_from(path: &Path) -> std::io::Result<ResumeState> {
    if !path.exists() {
        return Ok(ResumeState::default());
    }
    let file = File::open(path)?;
    let mut state = ResumeState::default();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(row) = parse_line(line.as_bytes()) {
            fold_seq_max(&mut state, &row);
        }
    }
    Ok(state)
}
