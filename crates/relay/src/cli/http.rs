use anyhow::{anyhow, Result};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Minimal blocking HTTP/1.1 client for localhost control calls — avoids pulling
/// in a full HTTP client crate for a few JSON round-trips.
pub fn get(addr: &str, path: &str) -> Result<String> {
    request(addr, "GET", path, None, 35)
}

pub fn post(addr: &str, path: &str, body: &str) -> Result<String> {
    request(addr, "POST", path, Some(body), 35)
}

/// POST with an explicit read timeout (seconds) — for slow backends like Ollama.
pub fn post_timeout(addr: &str, path: &str, body: &str, secs: u64) -> Result<String> {
    request(addr, "POST", path, Some(body), secs)
}

fn request(addr: &str, method: &str, path: &str, body: Option<&str>, read_secs: u64) -> Result<String> {
    let mut stream = TcpStream::connect(addr)
        .map_err(|e| anyhow!("cannot reach server at {addr}: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(read_secs)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.flush()?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    match text.split_once("\r\n\r\n") {
        Some((_, body)) => Ok(body.to_string()),
        None => Ok(String::new()),
    }
}
