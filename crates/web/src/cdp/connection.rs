use std::collections::VecDeque;

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::cdp_event::CdpEvent;
use super::incoming::Incoming;
use super::protocol_error::protocol_error;
use crate::transport::cdp_transport::CdpTransport;
use crate::transport::transport_error::transport_failed;

const MAX_BUFFERED_EVENTS: usize = 1024;

/// A CDP client over one transport. Requests carry monotonic ids; events
/// that arrive while a reply is awaited are buffered, oldest dropped first
/// once the buffer holds 1024.
pub struct CdpConnection {
    transport: Box<dyn CdpTransport>,
    next_id: u64,
    events: VecDeque<CdpEvent>,
    dropped_events: u64,
}

impl CdpConnection {
    pub fn new(transport: Box<dyn CdpTransport>) -> Self {
        Self {
            transport,
            next_id: 1,
            events: VecDeque::new(),
            dropped_events: 0,
        }
    }

    /// Sends `method` and waits for its reply. A flattened target session is
    /// addressed with `session_id`. A CDP error reply becomes an adapter
    /// error carrying the CDP message in `platform_detail`.
    pub fn call(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
        deadline: Deadline,
    ) -> Result<Value, AdapterError> {
        let id = self.next_id;
        self.next_id += 1;
        let mut request = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            request["sessionId"] = Value::String(session_id.to_owned());
        }
        self.transport.send_text(&request.to_string(), deadline)?;
        loop {
            let text = self.transport.receive_text(deadline)?;
            match Incoming::parse(&text) {
                Some(Incoming::Reply { id: reply_id, body }) if reply_id == id => {
                    return reply_result(method, body);
                }
                Some(Incoming::Event(event)) => self.buffer(event),
                Some(Incoming::Reply { .. } | Incoming::Unrecognized) => {}
                None => return Err(transport_failed(method, "unparseable DevTools frame")),
            }
        }
    }

    /// Returns the first event, buffered or newly read, that `matches`,
    /// leaving every other buffered event in place.
    pub fn wait_for_event(
        &mut self,
        mut matches: impl FnMut(&CdpEvent) -> bool,
        deadline: Deadline,
    ) -> Result<CdpEvent, AdapterError> {
        if let Some(index) = self.events.iter().position(&mut matches) {
            if let Some(event) = self.events.remove(index) {
                return Ok(event);
            }
        }
        loop {
            let text = self.transport.receive_text(deadline)?;
            match Incoming::parse(&text) {
                Some(Incoming::Event(event)) if matches(&event) => return Ok(event),
                Some(Incoming::Event(event)) => self.buffer(event),
                Some(_) => {}
                None => return Err(transport_failed("event", "unparseable DevTools frame")),
            }
        }
    }

    /// Drains every buffered event.
    pub fn take_events(&mut self) -> Vec<CdpEvent> {
        self.events.drain(..).collect()
    }

    /// Discards buffered events that `matches`.
    pub fn discard_events(&mut self, mut matches: impl FnMut(&CdpEvent) -> bool) {
        self.events.retain(|event| !matches(event));
    }

    /// How many events were discarded because the buffer was full.
    pub fn dropped_event_count(&self) -> u64 {
        self.dropped_events
    }

    fn buffer(&mut self, event: CdpEvent) {
        if self.events.len() >= MAX_BUFFERED_EVENTS {
            self.events.pop_front();
            self.dropped_events += 1;
        }
        self.events.push_back(event);
    }
}

fn reply_result(method: &str, mut body: Value) -> Result<Value, AdapterError> {
    if let Some(error) = body.get("error") {
        return Err(protocol_error(method, error));
    }
    Ok(body
        .get_mut("result")
        .map_or_else(|| json!({}), Value::take))
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
