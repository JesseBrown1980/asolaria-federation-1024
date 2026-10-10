//! asios-claude-bridge — connects the ASI OS desktop's Claude terminal (COM2 of the parallel
//! QEMU VM, exposed by QEMU on 127.0.0.1) to the operator's own Claude Code harness.
//!
//! Each `ASK|<text>` line from the VM runs `claude -p <text> --model <model>` in the operator's
//! signed-in harness (Asolaria MCPs, FABLE5 hooks, CLAUDE.md), continuing one session per bridge
//! run, and streams the answer back as `OUT|<line>` rows, then `END|rc=..|ms=..`.
//! Credentials never leave the harness: the VM only ever sees answer text.
//!
//! Always-on: reconnects every 2 s while the VM is down; one question at a time.
//! Journal: one `.hbp` row per event with sha256 of question/answer (`json=0`), hot path first.
//! The bridge listens on nothing; it is a client of the VM's loopback-only serial socket.
//! std only, integer only. E=0: it runs only when the operator starts it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_ADDR: &str = "127.0.0.1:4797";
const DEFAULT_MODEL: &str = "claude-opus-5-5";
const DEFAULT_JOURNAL: &str =
    "D:/PID-Registration-Office/offices/FABLE5-8467a937cba309f7/asios-parallel/claude-bridge.hbp";
const ANSWER_LIMIT: Duration = Duration::from_secs(900);
const CHUNK: usize = 400;

struct Cfg {
    addr: String,
    model: String,
    claude: String,
    journal: String,
    cwd: String,
}

fn cfg() -> Cfg {
    let mut c = Cfg {
        addr: DEFAULT_ADDR.into(),
        model: DEFAULT_MODEL.into(),
        claude: "claude".into(),
        journal: DEFAULT_JOURNAL.into(),
        cwd: std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into()),
    };
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i + 1 < a.len() {
        let v = a[i + 1].clone();
        match a[i].as_str() {
            "--addr" => c.addr = v,
            "--model" => c.model = v,
            "--claude" => c.claude = v,
            "--journal" => c.journal = v,
            "--cwd" => c.cwd = v,
            _ => {}
        }
        i += 2;
    }
    c
}

// ------------------------------------------------------------------ sha256 (FIPS 180-4)
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut m = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for blk in m.chunks(64) {
        let mut w = [0u32; 64];
        for t in 0..16 {
            w[t] = u32::from_be_bytes([blk[4 * t], blk[4 * t + 1], blk[4 * t + 2], blk[4 * t + 3]]);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for t in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn hex(b: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push(H[(x >> 4) as usize] as char);
        s.push(H[(x & 15) as usize] as char);
    }
    s
}

fn sha16(b: &[u8]) -> String {
    hex(&sha256(b)[..8])
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn journal(path: &str, row: &str) {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "CLAUDE-BRIDGE|{row}|unix_ms={}|json=0", unix_ms());
    }
    eprintln!("CLAUDE-BRIDGE|{row}");
}

/// A v4-shaped UUID from time + pid through a sha256 mix (no RNG crate needed).
fn session_uuid() -> String {
    let seed = format!("{}|{}|asios-claude-bridge", unix_ms(), std::process::id());
    let mut b = sha256(seed.as_bytes());
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b[..16]);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// The kernel's font is printable ASCII: map common Unicode punctuation, drop the rest to '?'.
fn ascii(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            ' '..='~' => o.push(ch),
            '\t' => o.push_str("    "),
            '\u{2018}' | '\u{2019}' | '\u{2032}' => o.push('\''),
            '\u{201c}' | '\u{201d}' | '\u{2033}' => o.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => o.push('-'),
            '\u{2022}' | '\u{00b7}' | '\u{2219}' => o.push('*'),
            '\u{2026}' => o.push_str("..."),
            '\u{2192}' => o.push_str("->"),
            '\u{2190}' => o.push_str("<-"),
            '\u{00a0}' => o.push(' '),
            '\u{00d7}' => o.push('x'),
            '\u{2264}' => o.push_str("<="),
            '\u{2265}' => o.push_str(">="),
            '\u{2260}' => o.push_str("!="),
            '\r' => {}
            _ => o.push('?'),
        }
    }
    o
}

fn send(s: &mut TcpStream, tag: &str, body: &str) -> std::io::Result<()> {
    let body = ascii(body);
    if body.is_empty() {
        return s.write_all(format!("{tag}|\n").as_bytes());
    }
    let bytes = body.as_bytes();
    for chunk in bytes.chunks(CHUNK) {
        s.write_all(tag.as_bytes())?;
        s.write_all(b"|")?;
        s.write_all(chunk)?;
        s.write_all(b"\n")?;
    }
    Ok(())
}

struct Session {
    id: String,
    started: bool,
}

fn answer(c: &Cfg, sess: &mut Session, q: &str, out: &mut TcpStream) -> std::io::Result<()> {
    let t0 = Instant::now();
    let mut cmd = Command::new(&c.claude);
    cmd.arg("-p")
        .arg(q)
        .arg("--model")
        .arg(&c.model)
        .arg("--output-format")
        .arg("text");
    if sess.started {
        cmd.arg("--resume").arg(&sess.id);
    } else {
        cmd.arg("--session-id").arg(&sess.id);
    }
    cmd.current_dir(&c.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(ch) => ch,
        Err(e) => {
            send(
                out,
                "OUT",
                &format!("bridge: cannot start '{}': {e}", c.claude),
            )?;
            send(out, "END", "rc=spawn_failed")?;
            journal(&c.journal, "op=spawn_failed");
            return Ok(());
        }
    };
    let stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        if let Some(so) = stdout {
            for line in BufReader::new(so).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    });
    let mut all = String::new();
    let mut timed_out = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                send(out, "OUT", &line)?;
                all.push_str(&line);
                all.push('\n');
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if t0.elapsed() > ANSWER_LIMIT {
                    let _ = child.kill();
                    timed_out = true;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = reader.join();
    let status = child.wait()?;
    let rc = status.code().unwrap_or(-1);
    let mut err = String::new();
    if let Some(se) = stderr.as_mut() {
        let _ = se.read_to_string(&mut err);
    }
    if rc != 0 {
        for l in err.lines().take(6) {
            send(out, "OUT", &format!("stderr: {l}"))?;
        }
    } else {
        sess.started = true;
    }
    let ms = t0.elapsed().as_millis();
    let st = if timed_out {
        format!("rc={rc}|timeout=1|ms={ms}")
    } else {
        format!("rc={rc}|ms={ms}")
    };
    send(out, "END", &st)?;
    journal(
        &c.journal,
        &format!(
            "op=answer|session={}|q_sha16={}|a_sha16={}|a_bytes={}|{st}",
            sess.id,
            sha16(q.as_bytes()),
            sha16(all.as_bytes()),
            all.len()
        ),
    );
    Ok(())
}

fn hello(c: &Cfg, sess: &Session) -> String {
    format!(
        "{} via the Claude Code harness of seat ACER-CLAUDE-FABLE5 (session {})",
        c.model,
        &sess.id[..8]
    )
}

fn serve(c: &Cfg, sess: &mut Session, s: TcpStream) -> std::io::Result<()> {
    let mut out = s.try_clone()?;
    send(&mut out, "HELLO", &hello(c, sess))?;
    let mut rd = BufReader::new(s);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if rd.read_until(b'\n', &mut raw)? == 0 {
            return Ok(()); // VM closed the socket
        }
        let line = String::from_utf8_lossy(&raw);
        let line = line.trim_end_matches(['\r', '\n']);
        let (tag, body) = line.split_once('|').unwrap_or((line, ""));
        match tag {
            "PING" => send(&mut out, "HELLO", &hello(c, sess))?,
            "ASK" => {
                let q = body.trim();
                if q.is_empty() {
                    send(&mut out, "END", "rc=empty_question")?;
                    continue;
                }
                journal(
                    &c.journal,
                    &format!("op=ask|session={}|q_sha16={}", sess.id, sha16(q.as_bytes())),
                );
                answer(c, sess, q, &mut out)?;
            }
            _ => {}
        }
    }
}

fn main() {
    let c = cfg();
    let mut sess = Session {
        id: session_uuid(),
        started: false,
    };
    journal(
        &c.journal,
        &format!(
            "op=start|addr={}|model={}|session={}",
            c.addr, c.model, sess.id
        ),
    );
    let mut waiting_logged = false;
    loop {
        match TcpStream::connect(&c.addr) {
            Ok(s) => {
                waiting_logged = false;
                let _ = s.set_nodelay(true);
                journal(&c.journal, &format!("op=connected|addr={}", c.addr));
                let r = serve(&c, &mut sess, s);
                journal(
                    &c.journal,
                    &format!(
                        "op=disconnected|why={}",
                        match r {
                            Ok(()) => "vm_closed".to_string(),
                            Err(e) => ascii(&e.to_string()).replace('|', "/"),
                        }
                    ),
                );
            }
            Err(_) => {
                if !waiting_logged {
                    journal(&c.journal, &format!("op=waiting_for_vm|addr={}", c.addr));
                    waiting_logged = true;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 56-byte message: padding spills into a second block.
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn uuid_shape() {
        let u = session_uuid();
        assert_eq!(u.len(), 36);
        assert_eq!(&u[14..15], "4");
        assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn ascii_maps_punctuation() {
        assert_eq!(
            ascii("a \u{2014} b \u{2192} \u{201c}c\u{201d} \u{2026}"),
            "a - b -> \"c\" ..."
        );
        assert_eq!(ascii("\u{4e2d}"), "?");
    }
}
