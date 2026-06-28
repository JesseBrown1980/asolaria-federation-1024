//! Minimal HTTP/1.1 read + response. Bounded single-read request-line parse (GET routes). Default
//! text/plain (json=0 HBP); a route may opt into application/json for the `?format=json` cold path.
//! `Connection: close` per request (thread-per-conn). No JSON on the hot path.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use crate::{routes, Shared};

pub fn handle(mut stream: TcpStream, shared: &Arc<Shared>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let mut buf = [0u8; 2048];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    let (code, ctype, body) = routes::route(shared, method, path, query);
    let _ = write(stream, code, ctype, &body);
}

pub fn write_text(stream: TcpStream, code: u16, body: &str) -> std::io::Result<()> {
    write(stream, code, "text/plain; charset=utf-8", body)
}

pub fn write(mut stream: TcpStream, code: u16, ctype: &str, body: &str) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}
