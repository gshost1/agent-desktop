use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::cdp_transport::CdpTransport;
use super::transport_error::transport_timeout;

/// A fake transport for unit tests. Each expectation names the CDP method
/// the next request must carry and the reply object (`result` or `error`)
/// to answer it with; unsolicited frames queue ahead of that reply. Every
/// request is logged so a test can inspect what was sent.
#[derive(Default)]
pub(crate) struct ScriptedTransport {
    expectations: VecDeque<(String, Vec<Value>, Value)>,
    inbound: VecDeque<String>,
    sent: Arc<Mutex<Vec<Value>>>,
}

impl ScriptedTransport {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn expect(self, method: &str, result: Value) -> Self {
        self.expect_with_events(method, Vec::new(), json!({ "result": result }))
    }

    pub(crate) fn expect_error(self, method: &str, code: i64, message: &str) -> Self {
        self.expect_with_events(
            method,
            Vec::new(),
            json!({ "error": { "code": code, "message": message } }),
        )
    }

    pub(crate) fn expect_with_events(
        mut self,
        method: &str,
        events: Vec<Value>,
        reply: Value,
    ) -> Self {
        self.expectations
            .push_back((method.to_owned(), events, reply));
        self
    }

    pub(crate) fn expect_silence(self, method: &str) -> Self {
        self.expect_with_events(method, Vec::new(), Value::Null)
    }

    pub(crate) fn push_inbound(mut self, frame: Value) -> Self {
        self.inbound.push_back(frame.to_string());
        self
    }

    pub(crate) fn sent_log(&self) -> Arc<Mutex<Vec<Value>>> {
        Arc::clone(&self.sent)
    }

    pub(crate) fn boxed(self) -> Box<dyn CdpTransport> {
        Box::new(self)
    }
}

impl CdpTransport for ScriptedTransport {
    fn send_text(&mut self, text: &str, _deadline: Deadline) -> Result<(), AdapterError> {
        let request: Value = serde_json::from_str(text).unwrap();
        self.sent.lock().unwrap().push(request.clone());
        let method = request["method"].as_str().unwrap_or_default().to_owned();
        let (expected, events, mut reply) = self
            .expectations
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected CDP request {method}"));
        assert_eq!(method, expected, "request order");
        for event in events {
            self.inbound.push_back(event.to_string());
        }
        if reply.is_null() {
            return Ok(());
        }
        reply["id"] = request["id"].clone();
        if let Some(session) = request.get("sessionId") {
            reply["sessionId"] = session.clone();
        }
        self.inbound.push_back(reply.to_string());
        Ok(())
    }

    fn receive_text(&mut self, _deadline: Deadline) -> Result<String, AdapterError> {
        self.inbound
            .pop_front()
            .ok_or_else(|| transport_timeout("receive"))
    }
}

impl Drop for ScriptedTransport {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let pending: Vec<&String> = self.expectations.iter().map(|(m, _, _)| m).collect();
            assert!(pending.is_empty(), "unsent expected requests: {pending:?}");
        }
    }
}
