use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, devtools_http_ok_body, read_devtools_http_response,
};

use super::connect_error::endpoint_unreachable;
use crate::transport::transport_error::{transport_failed, transport_timeout};

const MIN_IO_SLICE: Duration = Duration::from_millis(1);

/// Issues one `GET` to `127.0.0.1:port` and returns the body of its `200`
/// response. Core's DevTools HTTP reader ends the read once Content-Length
/// bytes of body arrive, because Chromium holds the connection open after
/// answering; every step is bounded by what remains of `deadline`.
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
    let raw = read_devtools_http_response(&mut stream, slice(deadline)?);
    let Some(raw) = raw else {
        if deadline.is_expired() {
            return Err(transport_timeout("http"));
        }
        return Err(transport_failed("http", "the server sent no response"));
    };
    devtools_http_ok_body(&raw).ok_or_else(|| transport_failed("http", "malformed HTTP response"))
}

fn slice(deadline: Deadline) -> Result<Duration, AdapterError> {
    if deadline.is_expired() {
        return Err(transport_timeout("http"));
    }
    Ok(deadline.remaining().max(MIN_IO_SLICE))
}
