use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use super::element_call::ElementCall;
use super::focus::focus;
use super::key_press::press_key;
use super::remote_element::call_on_element;
use crate::{CdpConnection, PageSession};

const SELECT_ALL: &str = r"function() {
  if ('value' in this && typeof this.select === 'function') { this.select(); return 'control'; }
  const doc = this.ownerDocument;
  const range = doc.createRange();
  range.selectNodeContents(this);
  const selection = doc.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
  return 'content';
}";

/// Replaces the node's text: focuses it, selects its whole content, then
/// inserts `text` as one input edit, as a paste would. Empty `text` deletes
/// the selection.
pub fn fill(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    text: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    focus(connection, session, backend_node_id, deadline)?;
    call_on_element(
        connection,
        session,
        backend_node_id,
        ElementCall {
            function: SELECT_ALL,
            arguments: Vec::new(),
        },
        deadline,
    )?;
    if text.is_empty() {
        return press_key(connection, session, "Backspace", deadline);
    }
    connection
        .call(
            "Input.insertText",
            json!({ "text": text }),
            Some(&session.session_id),
            deadline,
        )
        .map(|_| ())
}
