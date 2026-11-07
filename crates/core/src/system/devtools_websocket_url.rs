use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::{AdapterError, DeliverySemantics, ErrorCode};

/// A `ws://` URL whose host is a loopback address. Parsing refuses every
/// other host, `wss://` (the transport carries no TLS), credentials in the
/// authority, and a missing path, so a connection can only ever reach a
/// DevTools server on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevtoolsWebSocketUrl {
    host: String,
    port: u16,
    path: String,
}

impl DevtoolsWebSocketUrl {
    pub fn parse(value: &str) -> Result<Self, AdapterError> {
        let rest = value
            .strip_prefix("ws://")
            .ok_or_else(|| refused(value, "only ws:// URLs are accepted"))?;
        let (authority, path) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => return Err(refused(value, "the URL has no path")),
        };
        if authority.contains(['@', '?', '#']) || path.len() < 2 {
            return Err(refused(value, "the URL authority or path is malformed"));
        }
        let (host, port_text) = authority
            .rsplit_once(':')
            .ok_or_else(|| refused(value, "the URL names no port"))?;
        let port = port_text
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| refused(value, "the URL port is invalid"))?;
        if !matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
            return Err(refused(value, "the host is not a loopback address"));
        }
        Ok(Self {
            host: host.to_owned(),
            port,
            path: path.to_owned(),
        })
    }

    pub fn loopback(port: u16, path: &str) -> Result<Self, AdapterError> {
        let normalized = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        Self::parse(&format!("ws://127.0.0.1:{port}{normalized}"))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn socket_addr(&self) -> SocketAddr {
        if self.host == "[::1]" {
            SocketAddr::from((Ipv6Addr::LOCALHOST, self.port))
        } else {
            SocketAddr::from((Ipv4Addr::LOCALHOST, self.port))
        }
    }

    pub fn as_url(&self) -> String {
        format!("ws://{}:{}{}", self.host, self.port, self.path)
    }
}

fn refused(value: &str, reason: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!("Refusing DevTools WebSocket URL: {reason}"),
    )
    .with_suggestion("Connect only to a DevTools endpoint on 127.0.0.1, localhost or [::1]")
    .with_details(serde_json::json!({ "kind": "cdp_url_refused", "url": value }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "devtools_websocket_url_tests.rs"]
mod tests;
