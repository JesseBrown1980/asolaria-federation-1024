use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::canon::{canonical_bytes, scalar_string, scalar_u64, sha16};
use super::json_tok::{PyRow, RawJson};
use super::{APPENDED_BY_DAEMON, GENESIS_PREV};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeState {
    pub head_seq: u64,
    pub head_hash: String,
}

impl Default for ResumeState {
    fn default() -> Self {
        Self {
            head_seq: 0,
            head_hash: String::from(GENESIS_PREV),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendOut {
    pub seq: u64,
    pub row_hash: String,
    pub antecedent_prev: String,
}

pub fn fold_seq_max(acc: &mut ResumeState, row: &PyRow) {
    let Some(seq) = row.seq_if_integer() else {
        return;
    };
    if seq <= acc.head_seq {
        return;
    }
    acc.head_seq = seq;
    acc.head_hash = row.row_hash().unwrap_or_else(|| String::from(GENESIS_PREV));
}

pub fn compute_append(head: &ResumeState, mut payload: PyRow, now: &str) -> (PyRow, AppendOut) {
    let next_seq = head.head_seq + 1;
    let antecedent_prev = head.head_hash.clone();

    let had_antecedents = payload.contains("antecedents");
    let had_ts = payload.contains("ts");
    let had_appended_by = payload.contains("appended_by_daemon");
    let antecedents = payload.get("antecedents").cloned();

    payload.remove("seq");
    payload.remove("row_hash");
    payload.set("seq", scalar_u64(next_seq));

    if !had_antecedents {
        payload.set(
            "antecedents",
            RawJson::Array(vec![scalar_string(&antecedent_prev)]),
        );
    } else if let Some(RawJson::Array(existing)) = antecedents {
        let present = existing.iter().any(|v| match v {
            RawJson::Scalar(s) => s == &scalar_string(&antecedent_prev).scalar_raw(),
            _ => false,
        });
        if !present {
            let mut values = Vec::with_capacity(existing.len() + 1);
            values.push(scalar_string(&antecedent_prev));
            values.extend(existing);
            payload.set("antecedents", RawJson::Array(values));
        }
    }
    if !had_ts {
        payload.set("ts", scalar_string(now));
    }
    if !had_appended_by {
        payload.set("appended_by_daemon", scalar_string(APPENDED_BY_DAEMON));
    }

    // Keep row_hash physically last in the persisted line.
    let row_hash = sha16(&canonical_bytes(&payload));
    payload.set("row_hash", scalar_string(&row_hash));

    (
        payload,
        AppendOut {
            seq: next_seq,
            row_hash,
            antecedent_prev,
        },
    )
}

trait RawScalar {
    fn scalar_raw(&self) -> String;
}

impl RawScalar for RawJson {
    fn scalar_raw(&self) -> String {
        match self {
            RawJson::Scalar(s) => s.clone(),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::py_parity::{canonical_bytes, parse_line};

    #[test]
    fn append_adds_seq_antecedent_and_hash() {
        let row = parse_line(br#"{"event": "X"}"#).expect("parse");
        let head = ResumeState {
            head_seq: 7,
            head_hash: String::from("abc123"),
        };
        let (row, out) = compute_append(&head, row, "2026-06-25T00:00:00.000Z");
        assert_eq!(out.seq, 8);
        assert_eq!(out.antecedent_prev, "abc123");
        assert!(row.contains("row_hash"));
        assert_eq!(out.row_hash.len(), 16);
        assert!(canonical_bytes(&row).contains("\"seq\": 8"));
    }
}
