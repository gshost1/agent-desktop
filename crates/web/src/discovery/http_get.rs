use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use super::connect_error::endpoint_unreachable;
use crate::transport::transport_error::{transport_failed, transport_timeout};

const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MIN_IO_SLICE: Duration = Duration::from_millis(1);

/// Issues one `GET` to `127.0.0.1:port` and returns the body. Chromium's
/// DevTools server holds the connection open after answering, so the read
/// ends once Content-Length bytes of body arrive rather than at EOF. Every
/// read is bounded by what remains of `deadline`.
pub(crate) fn http_get_loopback(
    port: u16,
    path: &str,
    deadline: Deadline,
) -> Result<String, AdapterError> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, slice(deadline)?)
        .map_err(|error| endpoint_unreachable(port, &error.to_string()))?;
    stream
        .set_write_timeout(Some(slice(deadline)?))
        .map_err(|error| transport_failed("http", error.to_string()))?;
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| transport_failed("http", error.to_string()))?;
    let raw = read_response(&mut stream, deadline)?;
    split_body(&raw).ok_or_else(|| transport_failed("http", "malformed HTTP response"))
}

fn slice(deadline: Deadline) -> Result<Duration, AdapterError> {
    if deadline.is_expired() {
        return Err(transport_timeout("http"));
    }
    Ok(deadline.remaining().max(MIN_IO_SLICE))
}

fn read_response(stream: &mut TcpStream, deadline: Deadline) -> Result<Vec<u8>, AdapterError> {
    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        if deadline.is_expired() {
            return Err(transport_timeout("http"));
        }
        stream
            .set_read_timeout(Some(slice(deadline)?))
            .map_err(|error| transport_failed("http", error.to_string()))?;
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(raw),
            Ok(received) => {
                raw.extend_from_slice(&chunk[..received]);
                if response_complete(&raw) {
                    return Ok(raw);
                }
                if raw.len() > MAX_RESPONSE_BYTES {
                    return Err(transport_failed("http", "response exceeds 64 KiB"));
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(transport_failed("http", error.to_string())),
        }
    }
}

pub(crate) fn response_complete(raw: &[u8]) -> bool {
    let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let body_received = raw.len() - (header_end + 4);
    content_length(&raw[..header_end]).is_some_and(|expected| body_received >= expected)
}

fn content_length(headers: &[u8]) -> Option<usize> {
    String::from_utf8_lossy(headers).lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    })
}

pub(crate) fn split_body(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status_ok = head
        .lines()
        .next()
        .is_some_and(|status| status.split_whitespace().nth(1) == Some("200"));
    status_ok.then(|| body.trim().to_owned())
}

#[cfg(test)]
#[path = "http_get_tests.rs"]
mod tests;
