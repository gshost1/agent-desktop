use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use super::element_call::ElementCall;
use super::focus::focus;
use super::remote_element::call_on_element;
use crate::{CdpConnection, ElementToken, PageSession};

const CARET_TO_END: &str = r"function() {
  if ('value' in this && typeof this.setSelectionRange === 'function') {
    try {
      const end = String(this.value).length;
      this.setSelectionRange(end, end);
    } catch (error) {}
    return 'control';
  }
  const doc = this.ownerDocument;
  const range = doc.createRange();
  range.selectNodeContents(this);
  range.collapse(false);
  const selection = doc.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
  return 'content';
}";

/// Types `text` at the end of the node's current content: focuses it, moves
/// the caret past the last character, then inserts `text` as one input edit.
/// A control that does not expose a selection keeps the caret where focus put
/// it.
pub(crate) fn append_text(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    text: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    focus(connection, session, token, deadline)?;
    call_on_element(
        connection,
        session,
        token,
        ElementCall {
            function: CARET_TO_END,
            arguments: Vec::new(),
        },
        deadline,
    )?;
    if text.is_empty() {
        return Ok(());
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

#[cfg(test)]
#[path = "append_text_tests.rs"]
mod tests;
