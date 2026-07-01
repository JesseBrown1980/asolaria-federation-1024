use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use crate::{routes, Shared};

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    pub body: Vec<u8>,
}

pub fn handle_conn(mut stream: TcpStream, shared: Arc<Shared>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    match read_request(&mut stream) {
        Ok(req) => {
            let (status, body) = routes::route(&shared, req);
            let _ = write_status(stream, status, &body);
        }
        Err(_) => {
            let _ = write_status(stream, 400, r#"{"ok":false,"error":"bad-request"}"#);
        }
    }
}

pub fn write_status(mut stream: TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        reason,
        body.as_bytes().len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if find_header_end(&buf).is_some() {
            break;
        }
        if buf.len() > 64 * 1024 {
            break;
        }
    }
    let hdr_end = find_header_end(&buf)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no header end"))?;
    let head = String::from_utf8_lossy(&buf[..hdr_end]);
    let mut lines = head.split("\r\n");
    let first = lines
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "empty request"))?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
    let mut content_len = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse::<usize>().unwrap_or(0);
            }
        }
    }
    let body_start = hdr_end + 4;
    while buf.len() < body_start + content_len {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let body = buf
        .get(body_start..body_start.saturating_add(content_len))
        .unwrap_or(&[])
        .to_vec();
    Ok(Request {
        method,
        path,
        query,
        body,
    })
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}
