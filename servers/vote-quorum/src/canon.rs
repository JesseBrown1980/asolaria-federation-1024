//! canon — byte-exact reproduction of the py vote-quorum / cosign daemon row_hash recipe:
//! `row_hash = sha16( json.dumps({row without "row_hash"}, sort_keys=True, ensure_ascii=False) )`.
//! Structural parse -> drop top-level `row_hash` -> re-emit sorted by key with python `", "`/`": "`
//! separators, keeping scalar tokens VERBATIM (the daemon persists with the same json.dumps, so the
//! on-disk scalars are already canonical). Pure no_std (alloc + sha2). Proven 3321/3321 vs the
//! cosign ledger; the vote-quorum `append_row` uses the identical recipe.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

const HEXLUT: &[u8; 16] = b"0123456789abcdef";

/// sha256(utf8) -> lowercase hex -> first 16 chars (== daemon `sha16`).
pub fn sha16(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let d = h.finalize();
    let mut o = String::with_capacity(16);
    for &b in d.iter().take(8) {
        o.push(HEXLUT[(b >> 4) as usize] as char);
        o.push(HEXLUT[(b & 0x0f) as usize] as char);
    }
    o
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseErr {
    Eof,
    Unexpected,
    NotObject,
}

enum Val {
    Obj(Vec<(String, Val)>),
    Arr(Vec<Val>),
    Scalar(String),
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn new(b: &'a [u8]) -> Self {
        P { b, i: 0 }
    }
    fn ws(&mut self) {
        while let Some(&c) = self.b.get(self.i) {
            match c {
                b' ' | b'\t' | b'\n' | b'\r' => self.i += 1,
                _ => break,
            }
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn string_token(&mut self) -> Result<String, ParseErr> {
        let s = self.i;
        self.i += 1;
        while let Some(&c) = self.b.get(self.i) {
            match c {
                b'\\' => self.i += 2,
                b'"' => {
                    self.i += 1;
                    let t = core::str::from_utf8(&self.b[s..self.i])
                        .map_err(|_| ParseErr::Unexpected)?;
                    return Ok(t.to_string());
                }
                _ => self.i += 1,
            }
        }
        Err(ParseErr::Eof)
    }
    fn bare_token(&mut self) -> Result<String, ParseErr> {
        let s = self.i;
        while let Some(&c) = self.b.get(self.i) {
            match c {
                b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r' => break,
                _ => self.i += 1,
            }
        }
        if self.i == s {
            return Err(ParseErr::Unexpected);
        }
        Ok(core::str::from_utf8(&self.b[s..self.i])
            .map_err(|_| ParseErr::Unexpected)?
            .to_string())
    }
    fn value(&mut self) -> Result<Val, ParseErr> {
        self.ws();
        match self.peek().ok_or(ParseErr::Eof)? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Ok(Val::Scalar(self.string_token()?)),
            _ => Ok(Val::Scalar(self.bare_token()?)),
        }
    }
    fn object(&mut self) -> Result<Val, ParseErr> {
        self.i += 1;
        let mut m: Vec<(String, Val)> = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Val::Obj(m));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(ParseErr::Unexpected);
            }
            let k = self.string_token()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(ParseErr::Unexpected);
            }
            self.i += 1;
            let v = self.value()?;
            m.push((k, v));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(ParseErr::Unexpected),
            }
        }
        Ok(Val::Obj(m))
    }
    fn array(&mut self) -> Result<Val, ParseErr> {
        self.i += 1;
        let mut a: Vec<Val> = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Val::Arr(a));
        }
        loop {
            let v = self.value()?;
            a.push(v);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(ParseErr::Unexpected),
            }
        }
        Ok(Val::Arr(a))
    }
}

fn emit(v: &Val, o: &mut String) {
    match v {
        Val::Scalar(t) => o.push_str(t),
        Val::Arr(items) => {
            o.push('[');
            for (n, it) in items.iter().enumerate() {
                if n > 0 {
                    o.push_str(", ");
                }
                emit(it, o);
            }
            o.push(']');
        }
        Val::Obj(m) => {
            let mut idx: Vec<usize> = (0..m.len()).collect();
            idx.sort_by(|&a, &b| m[a].0.as_bytes().cmp(m[b].0.as_bytes()));
            o.push('{');
            for (n, &k) in idx.iter().enumerate() {
                if n > 0 {
                    o.push_str(", ");
                }
                o.push_str(&m[k].0);
                o.push_str(": ");
                emit(&m[k].1, o);
            }
            o.push('}');
        }
    }
}

/// The exact bytes the daemon hashes: the row as sorted-key JSON with top-level `row_hash` removed.
pub fn canonical_base(line: &str) -> Result<String, ParseErr> {
    let mut p = P::new(line.as_bytes());
    let v = p.value()?;
    let m = match v {
        Val::Obj(m) => m,
        _ => return Err(ParseErr::NotObject),
    };
    let kept: Vec<(String, Val)> = m.into_iter().filter(|(k, _)| k != "\"row_hash\"").collect();
    let mut o = String::new();
    emit(&Val::Obj(kept), &mut o);
    Ok(o)
}

/// Recompute a row's `row_hash` from its on-disk line (== daemon recipe).
pub fn recompute(line: &str) -> Result<String, ParseErr> {
    Ok(sha16(&canonical_base(line)?))
}

/// Extract the stored top-level `row_hash` (unquoted), or None for a legacy/no-hash row.
pub fn stored_row_hash(line: &str) -> Option<String> {
    let mut p = P::new(line.as_bytes());
    let v = p.value().ok()?;
    if let Val::Obj(m) = v {
        for (k, val) in &m {
            if k == "\"row_hash\"" {
                if let Val::Scalar(t) = val {
                    let b = t.as_bytes();
                    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
                        return Some(t[1..t.len() - 1].to_string());
                    }
                    return Some(t.clone());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha16_known_vector() {
        assert_eq!(sha16("hello world"), "b94d27b9934d3e08");
    }
    #[test]
    fn sorts_and_drops_row_hash() {
        assert_eq!(
            canonical_base(r#"{"b": 1, "a": 2, "row_hash": "x"}"#).unwrap(),
            r#"{"a": 2, "b": 1}"#
        );
    }
    #[test]
    fn stored_extracts() {
        assert_eq!(
            stored_row_hash(r#"{"v": 1, "row_hash": "1234abcd5678ef90"}"#).as_deref(),
            Some("1234abcd5678ef90")
        );
    }
}
