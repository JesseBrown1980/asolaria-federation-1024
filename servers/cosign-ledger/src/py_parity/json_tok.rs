use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawJson {
    Object(Vec<(String, RawJson)>),
    Array(Vec<RawJson>),
    Scalar(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyRow {
    pub fields: Vec<(String, RawJson)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PyChainErr {
    Parse,
    NonInteger,
    ChainBroken,
    Io,
}

pub fn parse_line(bytes: &[u8]) -> Result<PyRow, PyChainErr> {
    let s = core::str::from_utf8(bytes).map_err(|_| PyChainErr::Parse)?;
    let mut p = Parser::new(s.trim());
    let value = p.parse_value()?;
    p.skip_ws();
    if p.pos != p.s.len() {
        return Err(PyChainErr::Parse);
    }
    match value {
        RawJson::Object(fields) => Ok(PyRow { fields }),
        _ => Err(PyChainErr::Parse),
    }
}

impl PyRow {
    pub fn get(&self, key: &str) -> Option<&RawJson> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn set(&mut self, key: &str, value: RawJson) {
        if let Some((_, v)) = self.fields.iter_mut().find(|(k, _)| k == key) {
            *v = value;
            return;
        }
        self.fields.push((key.to_string(), value));
    }

    pub fn remove(&mut self, key: &str) {
        self.fields.retain(|(k, _)| k != key);
    }

    pub fn seq_if_integer(&self) -> Option<u64> {
        match self.get("seq") {
            Some(RawJson::Scalar(s)) => parse_u64_token(s),
            _ => None,
        }
    }

    pub fn row_hash(&self) -> Option<String> {
        match self.get("row_hash") {
            Some(RawJson::Scalar(s)) => string_scalar_value(s),
            _ => None,
        }
    }
}

pub fn string_scalar_value(raw: &str) -> Option<String> {
    if !(raw.starts_with('"') && raw.ends_with('"')) {
        return None;
    }
    decode_json_string(&raw[1..raw.len().saturating_sub(1)]).ok()
}

pub fn parse_u64_token(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<u64>().ok()
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Self { s, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\n' | b'\r' | b'\t') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn parse_value(&mut self) -> Result<RawJson, PyChainErr> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => {
                let raw = self.take_string_raw()?;
                Ok(RawJson::Scalar(raw))
            }
            Some(b'-' | b'0'..=b'9') => self.take_number_raw().map(RawJson::Scalar),
            Some(b't') => self.take_keyword("true").map(RawJson::Scalar),
            Some(b'f') => self.take_keyword("false").map(RawJson::Scalar),
            Some(b'n') => self.take_keyword("null").map(RawJson::Scalar),
            _ => Err(PyChainErr::Parse),
        }
    }

    fn parse_object(&mut self) -> Result<RawJson, PyChainErr> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(RawJson::Object(fields));
        }
        loop {
            self.skip_ws();
            let key_raw = self.take_string_raw()?;
            let key = string_scalar_value(&key_raw).ok_or(PyChainErr::Parse)?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.parse_value()?;
            if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k == &key) {
                *old = value;
            } else {
                fields.push((key, value));
            }
            self.skip_ws();
            match self.bump() {
                Some(b',') => {}
                Some(b'}') => break,
                _ => return Err(PyChainErr::Parse),
            }
        }
        Ok(RawJson::Object(fields))
    }

    fn parse_array(&mut self) -> Result<RawJson, PyChainErr> {
        self.expect(b'[')?;
        let mut values = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(RawJson::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => {}
                Some(b']') => break,
                _ => return Err(PyChainErr::Parse),
            }
        }
        Ok(RawJson::Array(values))
    }

    fn expect(&mut self, want: u8) -> Result<(), PyChainErr> {
        match self.bump() {
            Some(got) if got == want => Ok(()),
            _ => Err(PyChainErr::Parse),
        }
    }

    fn take_keyword(&mut self, kw: &str) -> Result<String, PyChainErr> {
        if self.s[self.pos..].starts_with(kw) {
            self.pos += kw.len();
            Ok(kw.to_string())
        } else {
            Err(PyChainErr::Parse)
        }
    }

    fn take_number_raw(&mut self) -> Result<String, PyChainErr> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        self.take_digits()?;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.take_digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.take_digits()?;
        }
        Ok(self.s[start..self.pos].to_string())
    }

    fn take_digits(&mut self) -> Result<(), PyChainErr> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(PyChainErr::Parse);
        }
        Ok(())
    }

    fn take_string_raw(&mut self) -> Result<String, PyChainErr> {
        let start = self.pos;
        self.expect(b'"')?;
        let mut escaped = false;
        while let Some(b) = self.bump() {
            if escaped {
                escaped = false;
                continue;
            }
            match b {
                b'\\' => escaped = true,
                b'"' => return Ok(self.s[start..self.pos].to_string()),
                0x00..=0x1f => return Err(PyChainErr::Parse),
                _ => {}
            }
        }
        Err(PyChainErr::Parse)
    }
}

fn decode_json_string(s: &str) -> Result<String, PyChainErr> {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match it.next().ok_or(PyChainErr::Parse)? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000c}'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                let v = take_hex4(&mut it)?;
                if (0xd800..=0xdbff).contains(&v) {
                    if it.next() != Some('\\') || it.next() != Some('u') {
                        return Err(PyChainErr::Parse);
                    }
                    let lo = take_hex4(&mut it)?;
                    if !(0xdc00..=0xdfff).contains(&lo) {
                        return Err(PyChainErr::Parse);
                    }
                    let scalar = 0x1_0000 + (((v - 0xd800) << 10) | (lo - 0xdc00));
                    out.push(char::from_u32(scalar).ok_or(PyChainErr::Parse)?);
                } else if (0xdc00..=0xdfff).contains(&v) {
                    return Err(PyChainErr::Parse);
                } else if let Some(c) = char::from_u32(v) {
                    out.push(c);
                } else {
                    return Err(PyChainErr::Parse);
                }
            }
            _ => return Err(PyChainErr::Parse),
        }
    }
    Ok(out)
}

fn take_hex4<I>(it: &mut I) -> Result<u32, PyChainErr>
where
    I: Iterator<Item = char>,
{
    let mut v = 0u32;
    for _ in 0..4 {
        let h = it.next().ok_or(PyChainErr::Parse)?;
        v = (v << 4) | h.to_digit(16).ok_or(PyChainErr::Parse)?;
    }
    Ok(v)
}
