use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::child_target::ChildTarget;
use crate::{CdpConnection, CdpEvent, PageSession};

const ATTACHED: &str = "Target.attachedToTarget";
const DETACHED: &str = "Target.detachedFromTarget";

/// Attaches a flattened session to every out-of-process iframe directly
/// beneath `session`'s target. Auto-attach reports the iframe targets that
/// already exist before it replies, and a target paused for a debugger is
/// released.
pub(crate) fn attach_child_targets(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<Vec<ChildTarget>, AdapterError> {
    set_auto_attach(connection, session, true, deadline)?;
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

/// Turns auto-attach back off for `session`, which detaches the iframe
/// sessions it attached, and drops the attach bookkeeping events. Best
/// effort: the snapshot is already read.
pub(crate) fn stop_child_targets(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) {
    let _ = set_auto_attach(connection, session, false, deadline);
    let parent = session.session_id.as_str();
    connection.discard_events(|event| {
        matches!(event.method.as_str(), ATTACHED | DETACHED)
            && event.session_id.as_deref() == Some(parent)
    });
}

fn set_auto_attach(
    connection: &mut CdpConnection,
    session: &PageSession,
    enabled: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    connection
        .call(
            "Target.setAutoAttach",
            json!({ "autoAttach": enabled, "waitForDebuggerOnStart": false, "flatten": true }),
            Some(&session.session_id),
            deadline,
        )
        .map(|_| ())
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
