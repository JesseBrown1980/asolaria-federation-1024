use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

use super::json_tok::{PyRow, RawJson};

pub fn sha16(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(16);
    for b in digest.iter().take(8) {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn canonical_bytes(row: &PyRow) -> String {
    object_bytes(
        row.fields
            .iter()
            .filter(|(k, _)| k != "row_hash")
            .map(|(k, v)| (k.as_str(), v))
            .collect(),
        true,
    )
}

pub fn persisted_line(row: &PyRow) -> String {
    let mut out = object_bytes(
        row.fields.iter().map(|(k, v)| (k.as_str(), v)).collect(),
        false,
    );
    out.push('\n');
    out
}

fn value_bytes(value: &RawJson, sort_object_keys: bool) -> String {
    match value {
        RawJson::Scalar(raw) => raw.clone(),
        RawJson::Array(values) => {
            let mut out = String::from("[");
            for (idx, value) in values.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                out.push_str(&value_bytes(value, sort_object_keys));
            }
            out.push(']');
            out
        }
        RawJson::Object(fields) => object_bytes(
            fields.iter().map(|(k, v)| (k.as_str(), v)).collect(),
            sort_object_keys,
        ),
    }
}

fn object_bytes(mut fields: Vec<(&str, &RawJson)>, sort_keys: bool) -> String {
    if sort_keys {
        fields.sort_by(|a, b| a.0.cmp(b.0));
    }
    let mut out = String::from("{");
    for (idx, (key, value)) in fields.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&py_escape_string(key));
        out.push_str(": ");
        out.push_str(&value_bytes(value, sort_keys));
    }
    out.push('}');
    out
}

pub fn py_escape_string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{000c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\u{0000}'..='\u{001f}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            '\u{0080}'..='\u{ffff}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            // Printable ASCII (U+0020..U+007F) not otherwise escaped: emit verbatim, matching
            // python json.dumps(ensure_ascii=False). This guards the U+10000+ surrogate-pair math
            // below from underflowing on BMP chars — a debug panic that crashed the /verify
            // recompute path on every ASCII row key (e.g. "seq", "row_hash").
            c if (c as u32) < 0x1_0000 => out.push(c),
            _ => {
                let v = ch as u32 - 0x1_0000;
                let hi = 0xd800 + ((v >> 10) & 0x3ff);
                let lo = 0xdc00 + (v & 0x3ff);
                out.push_str(&format!("\\u{hi:04x}\\u{lo:04x}"));
            }
        }
    }
    out.push('"');
    out
}

pub fn scalar_string(s: &str) -> RawJson {
    RawJson::Scalar(py_escape_string(s))
}

pub fn scalar_u64(v: u64) -> RawJson {
    RawJson::Scalar(v.to_string())
}
