use std::io::ErrorKind;
use std::net::TcpStream;
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};
use tungstenite::client::IntoClientRequest;
use tungstenite::handshake::HandshakeError;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Error as WsError, Message, WebSocket};

use super::cdp_transport::CdpTransport;
use super::transport_error::{transport_closed, transport_failed, transport_timeout};
use crate::WebSocketUrl;
use crate::discovery::connect_error::handshake_rejected;

const MAX_MESSAGE_BYTES: usize = 256 * 1024 * 1024;
const MIN_IO_SLICE: Duration = Duration::from_millis(1);

/// A blocking WebSocket client to a loopback DevTools server. Large
/// messages such as a full accessibility tree are accepted up to 256 MiB.
#[derive(Debug)]
pub(crate) struct WebSocketTransport {
    socket: WebSocket<TcpStream>,
}

impl WebSocketTransport {
    /// Opens the TCP connection and completes the WebSocket upgrade before
    /// `deadline`. A server that holds the upgrade (a browser waiting on its
    /// user's approval) costs at most the remaining deadline.
    pub(crate) fn connect(url: &WebSocketUrl, deadline: Deadline) -> Result<Self, AdapterError> {
        let stream = TcpStream::connect_timeout(&url.socket_addr(), io_slice(deadline)?)
            .map_err(|error| connect_failed(url, &error))?;
        stream
            .set_nodelay(true)
            .map_err(|error| transport_failed("connect", error.to_string()))?;
        apply_timeouts(&stream, deadline)?;
        let request = url
            .as_url()
            .into_client_request()
            .map_err(|error| transport_failed("handshake", error.to_string()))?;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let mut pending = tungstenite::client::client_with_config(request, stream, Some(config));
        loop {
            match pending {
                Ok((socket, _)) => return Ok(Self { socket }),
                Err(HandshakeError::Interrupted(mut mid)) => {
                    if deadline.is_expired() {
                        return Err(transport_timeout("handshake"));
                    }
                    apply_timeouts(mid.get_mut().get_mut(), deadline)?;
                    pending = mid.handshake();
                }
                Err(HandshakeError::Failure(WsError::Http(response))) => {
                    return Err(handshake_rejected(response.status().as_u16(), url.port()));
                }
                Err(HandshakeError::Failure(error)) => {
                    return Err(map_ws_error("handshake", error));
                }
            }
        }
    }
}

impl CdpTransport for WebSocketTransport {
    fn send_text(&mut self, text: &str, deadline: Deadline) -> Result<(), AdapterError> {
        apply_timeouts(self.socket.get_ref(), deadline)?;
        self.socket
            .send(Message::text(text))
            .map_err(|error| map_ws_error("send", error))
    }

    fn receive_text(&mut self, deadline: Deadline) -> Result<String, AdapterError> {
        loop {
            apply_timeouts(self.socket.get_ref(), deadline)?;
            match self.socket.read() {
                Ok(Message::Text(text)) => return Ok(text.as_str().to_owned()),
                Ok(Message::Binary(bytes)) => {
                    return String::from_utf8(bytes.to_vec())
                        .map_err(|error| transport_failed("receive", error.to_string()));
                }
                Ok(Message::Close(frame)) => {
                    return Err(transport_closed(frame.map_or_else(String::new, |frame| {
                        frame.reason.as_str().to_owned()
                    })));
                }
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
                Err(WsError::Io(error)) if is_retry(error.kind()) => {
                    if deadline.is_expired() {
                        return Err(transport_timeout("receive"));
                    }
                }
                Err(error) => return Err(map_ws_error("receive", error)),
            }
        }
    }
}

fn is_retry(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
    )
}

fn io_slice(deadline: Deadline) -> Result<Duration, AdapterError> {
    if deadline.is_expired() {
        return Err(transport_timeout("connect"));
    }
    Ok(deadline.remaining().max(MIN_IO_SLICE))
}

fn apply_timeouts(stream: &TcpStream, deadline: Deadline) -> Result<(), AdapterError> {
    let slice = Some(deadline.remaining().max(MIN_IO_SLICE));
    stream
        .set_read_timeout(slice)
        .and_then(|()| stream.set_write_timeout(slice))
        .map_err(|error| transport_failed("configure", error.to_string()))
}

fn map_ws_error(operation: &str, error: WsError) -> AdapterError {
    match error {
        WsError::ConnectionClosed | WsError::AlreadyClosed => transport_closed(error.to_string()),
        WsError::Io(io) if is_retry(io.kind()) => transport_timeout(operation),
        WsError::Io(io) if io.kind() == ErrorKind::ConnectionReset => {
            transport_closed(io.to_string())
        }
        other => transport_failed(operation, other.to_string()),
    }
}

/// Every endpoint is loopback, so a TCP connect that does not complete means
/// nothing accepted it. Windows retries a refused loopback SYN for about two
/// seconds before it reports the refusal, so a deadline shorter than that ends
/// as a timeout there; both outcomes are the same unavailable endpoint.
fn connect_failed(url: &WebSocketUrl, error: &std::io::Error) -> AdapterError {
    crate::discovery::connect_error::endpoint_unreachable(url.port(), &error.to_string())
}

#[cfg(test)]
#[path = "websocket_tests.rs"]
mod tests;
