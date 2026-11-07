use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::frame_entry::FrameEntry;
use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession};

/// Lists the target's frames in pre-order; the target's root frame comes
/// first. A page's main frame has no parent. An out-of-process iframe
/// target's root names its parent frame, which lives in another target.
pub(crate) fn list_frames(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<Vec<FrameEntry>, AdapterError> {
    let result = connection.call(
        "Page.getFrameTree",
        json!({}),
        Some(&session.session_id),
        deadline,
    )?;
    let root = result
        .get("frameTree")
        .ok_or_else(|| transport_failed("Page.getFrameTree", "reply has no frameTree"))?;
    let root_parent = text(root, "/frame/parentId");
    let mut frames = Vec::new();
    let mut pending = vec![(root, root_parent)];
    while let Some((tree, parent_id)) = pending.pop() {
        let Some(frame_id) = text(tree, "/frame/id") else {
            continue;
        };
        if let Some(children) = tree.get("childFrames").and_then(Value::as_array) {
            pending.extend(
                children
                    .iter()
                    .rev()
                    .map(|child| (child, Some(frame_id.clone()))),
            );
        }
        frames.push(FrameEntry {
            loader_id: text(tree, "/frame/loaderId").unwrap_or_default(),
            frame_id,
            parent_id,
        });
    }
    Ok(frames)
}

/// The loader of the document `frame_id` shows now, or `None` when the
/// target no longer has that frame.
pub(crate) fn current_loader(
    connection: &mut CdpConnection,
    session: &PageSession,
    frame_id: &str,
    deadline: Deadline,
) -> Result<Option<String>, AdapterError> {
    Ok(list_frames(connection, session, deadline)?
        .into_iter()
        .find(|frame| frame.frame_id == frame_id)
        .map(|frame| frame.loader_id))
}

fn text(tree: &Value, pointer: &str) -> Option<String> {
    tree.pointer(pointer)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
