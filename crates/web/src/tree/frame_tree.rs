use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession};

/// Lists the page's frames in pre-order as `(frame_id, parent_frame_id)`;
/// the main frame comes first and has no parent.
pub(crate) fn list_frames(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<Vec<(String, Option<String>)>, AdapterError> {
    let result = connection.call(
        "Page.getFrameTree",
        json!({}),
        Some(&session.session_id),
        deadline,
    )?;
    let root = result
        .get("frameTree")
        .ok_or_else(|| transport_failed("Page.getFrameTree", "reply has no frameTree"))?;
    let mut frames = Vec::new();
    let mut pending = vec![(root, None::<String>)];
    while let Some((tree, parent)) = pending.pop() {
        let Some(frame_id) = tree.pointer("/frame/id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(children) = tree.get("childFrames").and_then(Value::as_array) {
            pending.extend(
                children
                    .iter()
                    .rev()
                    .map(|child| (child, Some(frame_id.to_owned()))),
            );
        }
        frames.push((frame_id.to_owned(), parent));
    }
    Ok(frames)
}
