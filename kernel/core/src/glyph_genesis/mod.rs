//! GLYPH-GENESIS Rust port · BEHCS-256 absorb
//!
//! Direct port of `C:/asolaria-behcs-256/GLYPH-GENESIS.js` (sha16=`b06bb7701f8794f7`, 5383B).
//! Author of source: Jesse Daniel Brown, 2026-04-16.
//!
//! Algorithm:
//!   1. SHA-256 of key bytes
//!   2. Read first 8 bytes as big-endian u64
//!   3. For i=0..7: out[i] = alphabet[v % 256]; v = v / 256
//!   4. Return concatenated string
//!
//! The 256-glyph alphabet is parameterized in v0.1 (caller provides). Real BEHCS-256 + BEHCS-1024
//! UTF-8 alphabet binding lands in v0.2 with the full 1024-glyph table.

use alloc::string::String;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

/// Canonical alphabet size per BEHCS-256 spec (Step 81 binding · matches JS reference).
pub const BEHCS256_ALPHABET_SIZE: usize = 256;

/// Glyph-genesis errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GlyphErr {
    /// Provided alphabet does not contain exactly 256 entries.
    AlphabetSizeMismatch,
}

/// Computes the deterministic 8-glyph string for `key` using the provided 256-entry alphabet.
///
/// Byte-parity with the JS reference: for the same key + same alphabet, this returns the same glyph string.
pub fn glyph(key: &str, alphabet: &[&str; BEHCS256_ALPHABET_SIZE]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();

    // Read first 8 bytes as big-endian u64 (matches JS readBigUInt64BE(0)).
    let mut v: u64 = 0;
    for i in 0..8 {
        v = (v << 8) | (digest[i] as u64);
    }

    let mut out = String::new();
    let base: u64 = BEHCS256_ALPHABET_SIZE as u64;
    for _ in 0..8 {
        out.push_str(alphabet[(v % base) as usize]);
        v /= base;
    }
    out
}

/// One sentence-word: dim + tuple + glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceWord {
    pub dim: String,
    pub tuple: String,
    pub glyph: String,
}

/// Compose a "sentence" from a set of (dim, value) tuples — each maps to one glyph-word.
///
/// Mirrors JS `sentence(fields)` → `[{dim, tuple, glyph}]`.
pub fn sentence(
    fields: &[(&str, &str)],
    alphabet: &[&str; BEHCS256_ALPHABET_SIZE],
) -> Vec<SentenceWord> {
    fields
        .iter()
        .map(|(d, v)| {
            let tuple = {
                let mut s = String::with_capacity(d.len() + 1 + v.len());
                s.push_str(d);
                s.push(':');
                s.push_str(v);
                s
            };
            let g = glyph(&tuple, alphabet);
            SentenceWord {
                dim: (*d).into(),
                tuple,
                glyph: g,
            }
        })
        .collect()
}

/// v0.1 test alphabet: first 95 printable ASCII + 161 placeholder slots filled with "<Gi>".
/// v0.2 will bind the real BEHCS-256 UTF-8 alphabet from `C:/asolaria-behcs-256/GLYPH-GENESIS.js`
/// ALPHABET constant (positions 95-255 contain Greek/math/box-drawing/symbol UTF-8 chars).
pub fn test_alphabet_v0_1() -> [&'static str; BEHCS256_ALPHABET_SIZE] {
    // ASCII printable 32..126 = 95 entries; pad the rest with placeholders.
    // For byte-parity testing we need actual real alphabet from JS reference.
    // v0.1 returns a simple deterministic placeholder; tests use this only for round-trip checking.
    const PLACEHOLDER: &str = "?";
    let mut arr: [&'static str; 256] = [PLACEHOLDER; 256];
    // Map index 0..95 to ASCII 32..126 (printable).
    const ASCII_TABLE: &[&str; 95] = &[
        " ", "!", "\"", "#", "$", "%", "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", "0", "1",
        "2", "3", "4", "5", "6", "7", "8", "9", ":", ";", "<", "=", ">", "?", "@", "A", "B", "C",
        "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U",
        "V", "W", "X", "Y", "Z", "[", "\\", "]", "^", "_", "`", "a", "b", "c", "d", "e", "f", "g",
        "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y",
        "z", "{", "|", "}", "~",
    ];
    let mut i = 0;
    while i < 95 {
        arr[i] = ASCII_TABLE[i];
        i += 1;
    }
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_size_is_256() {
        assert_eq!(BEHCS256_ALPHABET_SIZE, 256);
    }

    #[test]
    fn glyph_is_8_words_long() {
        let alpha = test_alphabet_v0_1();
        let g = glyph("D1:liris", &alpha);
        // 8 glyph-slots; each slot is one alphabet entry — but entries can be multi-char.
        // For v0.1 ASCII-only alphabet, each entry is single char so output is 8 chars.
        assert_eq!(g.chars().count(), 8);
    }

    #[test]
    fn glyph_deterministic() {
        let alpha = test_alphabet_v0_1();
        let g1 = glyph("D1:liris", &alpha);
        let g2 = glyph("D1:liris", &alpha);
        assert_eq!(g1, g2);
    }

    #[test]
    fn different_keys_produce_different_glyphs() {
        let alpha = test_alphabet_v0_1();
        let g1 = glyph("D1:liris", &alpha);
        let g2 = glyph("D1:falcon", &alpha);
        assert_ne!(g1, g2);
    }

    #[test]
    fn sentence_returns_one_word_per_field() {
        let alpha = test_alphabet_v0_1();
        let s = sentence(
            &[("D1", "liris"), ("D2", "heartbeat"), ("D7", "alive")],
            &alpha,
        );
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].dim, "D1");
        assert_eq!(s[0].tuple, "D1:liris");
        assert_eq!(s[0].glyph.chars().count(), 8);
    }
}
