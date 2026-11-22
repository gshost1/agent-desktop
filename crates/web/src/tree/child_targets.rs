use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::child_target::ChildTarget;
use crate::cdp::cdp_request::CdpRequest;
use crate::{CdpConnection, CdpEvent, PageSession};

const ATTACHED: &str = "Target.attachedToTarget";
const DETACHED: &str = "Target.detachedFromTarget";

/// Attaches a flattened session to every out-of-process iframe directly
/// beneath each parent's target, asking all parents at once, and returns
/// each parent's children in parent order. Auto-attach reports the iframe
/// targets that already exist before it replies, and a target paused for a
/// debugger is released.
pub(crate) fn attach_child_targets(
    connection: &mut CdpConnection,
    parents: &[(PageSession, Deadline)],
) -> Result<Vec<Result<Vec<ChildTarget>, AdapterError>>, AdapterError> {
    let requests: Vec<CdpRequest> = parents
        .iter()
        .map(|(session, deadline)| auto_attach(session, true, *deadline))
        .collect();
    let replies = connection.call_batch(&requests)?;
    Ok(parents
        .iter()
        .zip(replies)
        .map(|((session, deadline), reply)| {
            reply.and_then(|_| attached_children(connection, session, *deadline))
        })
        .collect())
}

fn attached_children(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<Vec<ChildTarget>, AdapterError> {
    let parent = session.session_id.as_str();
    let attached = connection.drain_events(|event| {
        event.method == ATTACHED
            && event.session_id.as_deref() == Some(parent)
            && event
                .params
                .pointer("/targetInfo/type")
                .and_then(Value::as_str)
                == Some("iframe")
    });
    let mut children = Vec::with_capacity(attached.len());
    for event in attached {
        let Some(child) = child_target(&event) else {
            continue;
        };
        if event
            .params
            .get("waitingForDebugger")
            .and_then(Value::as_bool)
            == Some(true)
        {
            connection.call(
                "Runtime.runIfWaitingForDebugger",
                json!({}),
                Some(&child.session.session_id),
                deadline,
            )?;
        }
        children.push(child);
    }
    Ok(children)
}

/// Turns auto-attach back off for each of `sessions`, in order and all at
/// once, which detaches the iframe sessions they attached, and drops the
/// attach bookkeeping events. Best effort: the snapshot is already read.
pub(crate) fn stop_child_targets(
    connection: &mut CdpConnection,
    sessions: &[PageSession],
    deadline: Deadline,
) {
    let requests: Vec<CdpRequest> = sessions
        .iter()
        .map(|session| auto_attach(session, false, deadline))
        .collect();
    let _ = connection.call_batch(&requests);
    connection.discard_events(|event| {
        matches!(event.method.as_str(), ATTACHED | DETACHED)
            && sessions
                .iter()
                .any(|session| event.session_id.as_deref() == Some(session.session_id.as_str()))
    });
}

fn auto_attach(session: &PageSession, enabled: bool, deadline: Deadline) -> CdpRequest {
    CdpRequest {
        method: "Target.setAutoAttach",
        params: json!({ "autoAttach": enabled, "waitForDebuggerOnStart": false, "flatten": true }),
        session_id: Some(session.session_id.clone()),
        deadline,
    }
}

fn child_target(event: &CdpEvent) -> Option<ChildTarget> {
    let text = |pointer: &str| {
        event
            .params
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    Some(ChildTarget {
        session: PageSession {
            target_id: text("/targetInfo/targetId")?,
            session_id: text("/sessionId")?,
        },
        parent_frame_id: text("/targetInfo/parentFrameId"),
    })
}
