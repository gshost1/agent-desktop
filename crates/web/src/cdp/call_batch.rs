//! Pipelined requests over one connection. The browser answers requests to
//! different targets, which render in different processes, concurrently; a
//! client that waits for each reply before sending the next request pays
//! every target's latency in turn.

use std::collections::HashMap;
use std::time::Instant;

use agent_desktop_core::{AdapterError, Deadline};

use super::cdp_request::CdpRequest;
use super::connection::{CdpConnection, reply_result, reply_unreadable, session_detached};
use super::incoming::Incoming;
use super::protocol_error::is_target_gone;
use crate::transport::transport_error::{is_timeout, transport_failed};

/// The most requests one batch keeps awaiting a reply at once.
pub(crate) const MAX_IN_FLIGHT: usize = 16;

type Replies = Vec<Option<Result<serde_json::Value, AdapterError>>>;

struct InFlight {
    index: usize,
    sent: Instant,
}

impl CdpConnection {
    /// Sends `requests` without waiting between them, keeping at most
    /// [`MAX_IN_FLIGHT`] unanswered, and returns each one's outcome in request
    /// order, whatever order the replies arrive in. A request whose deadline
    /// passes, whose session is detached, or that the browser refuses fails on
    /// its own as [`CdpConnection::call`] would fail it; a transport failure
    /// fails the batch.
    pub(crate) fn call_batch(
        &mut self,
        requests: &[CdpRequest],
    ) -> Result<Vec<Result<serde_json::Value, AdapterError>>, AdapterError> {
        let mut replies: Replies = vec![None; requests.len()];
        let mut in_flight: HashMap<u64, InFlight> = HashMap::new();
        let mut next = 0;
        while next < requests.len() || !in_flight.is_empty() {
            while next < requests.len() && in_flight.len() < MAX_IN_FLIGHT {
                let request = &requests[next];
                let sent = Instant::now();
                match self.send_request(
                    request.method,
                    request.params.clone(),
                    request.session_id.as_deref(),
                    request.deadline,
                ) {
                    Ok(id) => {
                        in_flight.insert(id, InFlight { index: next, sent });
                    }
                    Err(error) if is_timeout(&error) || is_target_gone(&error) => {
                        replies[next] = Some(Err(error));
                    }
                    Err(error) => return Err(error),
                }
                next += 1;
            }
            let Some(deadline) = earliest_deadline(requests, &in_flight) else {
                continue;
            };
            match self.receive_incoming(deadline) {
                Ok(Some(incoming)) => {
                    self.settle(incoming, requests, &mut in_flight, &mut replies);
                }
                Ok(None) => return Err(transport_failed("batch", "unparseable DevTools frame")),
                Err(error) if is_timeout(&error) => {
                    in_flight.retain(|_, pending| {
                        let request_deadline = requests[pending.index].deadline;
                        let missed = request_deadline == deadline || request_deadline.is_expired();
                        if missed {
                            replies[pending.index] = Some(Err(error.clone()));
                        }
                        !missed
                    });
                }
                Err(error) => return Err(error),
            }
        }
        Ok(replies
            .into_iter()
            .zip(requests)
            .map(|(reply, request)| {
                reply.unwrap_or_else(|| Err(transport_failed(request.method, "no reply recorded")))
            })
            .collect())
    }

    fn settle(
        &mut self,
        incoming: Incoming,
        requests: &[CdpRequest],
        in_flight: &mut HashMap<u64, InFlight>,
        replies: &mut Replies,
    ) {
        match incoming {
            Incoming::Reply { id, body } => {
                if let Some(pending) = in_flight.remove(&id) {
                    let method = requests[pending.index].method;
                    let reply = reply_result(method, body);
                    trace_reply(method, &pending, reply.is_ok());
                    replies[pending.index] = Some(reply);
                }
            }
            Incoming::UnreadableReply { id } => {
                if let Some(pending) = in_flight.remove(&id) {
                    let method = requests[pending.index].method;
                    trace_reply(method, &pending, false);
                    replies[pending.index] = Some(Err(reply_unreadable(method)));
                }
            }
            Incoming::Event(event) => {
                self.buffer(event);
                in_flight.retain(|_, pending| {
                    let request = &requests[pending.index];
                    let detached = request
                        .session_id
                        .as_deref()
                        .is_some_and(|session| self.is_detached(session));
                    if detached {
                        replies[pending.index] = Some(Err(session_detached(request.method)));
                    }
                    !detached
                });
            }
            Incoming::Unrecognized => {}
        }
    }
}

fn earliest_deadline(
    requests: &[CdpRequest],
    in_flight: &HashMap<u64, InFlight>,
) -> Option<Deadline> {
    in_flight
        .values()
        .map(|pending| requests[pending.index].deadline)
        .min_by_key(|deadline| deadline.remaining())
}

fn trace_reply(method: &str, pending: &InFlight, ok: bool) {
    tracing::debug!(
        method,
        elapsed_ms = pending.sent.elapsed().as_secs_f64() * 1_000.0,
        ok,
        "cdp call"
    );
}

#[cfg(test)]
#[path = "call_batch_tests.rs"]
mod tests;
