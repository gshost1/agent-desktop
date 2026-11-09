use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::element_call::ElementCall;
use super::fill_outcome::FillOutcome;
use super::input_value::{assign_value, part_of_input};
use super::input_value_kind::InputValueKind;
use super::key_press::press_key;
use super::remote_element::{call_function, with_element};
use super::value_target::ValueTarget;
use crate::{CdpConnection, ElementToken, PageSession};

const PREPARE_FILL: &str = r"function() {
  const TEXT_TYPES = ['text', 'search', 'url', 'tel', 'email', 'password', 'number'];
  const VALUE_TYPES = ['date', 'time', 'datetime-local', 'month', 'week', 'color', 'range'];
  const tag = this.localName;
  if (tag === 'input' && VALUE_TYPES.includes(this.type)) {
    if (this.disabled || this.readOnly) return { editable: false, reason: this.disabled ? 'disabled' : 'read-only' };
    return { valueType: this.type };
  }
  const root = this.getRootNode ? this.getRootNode() : null;
  const host = root && root !== this ? root.host : null;
  if (host && host.localName === 'input' && VALUE_TYPES.includes(host.type)) return { editable: false, partOf: host.type };
  const control = tag === 'textarea' || (tag === 'input' && TEXT_TYPES.includes(this.type));
  if (tag === 'input' && !control) return { editable: false, reason: 'input type ' + this.type };
  if (control && (this.disabled || this.readOnly)) return { editable: false, reason: this.disabled ? 'disabled' : 'read-only' };
  if (!control && !this.isContentEditable) return { editable: false, reason: 'not editable' };
  this.focus();
  if (control) {
    try { this.select(); } catch (error) { return { editable: false, reason: 'no text selection' }; }
    return { editable: true };
  }
  const doc = this.ownerDocument;
  const range = doc.createRange();
  range.selectNodeContents(this);
  const selection = doc.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
  return { editable: true };
}";

const READ_FILLED: &str = r"function() {
  if (this.localName === 'input' || this.localName === 'textarea') return String(this.value);
  const text = this.innerText !== undefined ? this.innerText : this.textContent;
  return text === null || text === undefined ? null : String(text);
}";

/// Replaces the node's text: focuses it, selects its whole content, inserts
/// `text` as one input edit, as a paste would, and reads the text back.
/// Empty `text` deletes the selection. A node that cannot take text (a
/// button, a checkbox, a disabled or read-only field) is
/// `ACTION_NOT_SUPPORTED` before anything is typed. An input that takes its
/// value whole (date, time, datetime-local, month, week, color, range) is
/// assigned `text` in its own format instead, and one part of such an input,
/// such as its hours field, is `ACTION_NOT_SUPPORTED` naming the input.
pub fn fill(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    text: &str,
    deadline: Deadline,
) -> Result<FillOutcome, AdapterError> {
    with_element(
        connection,
        session,
        token,
        deadline,
        |connection, object| {
            let prepared =
                call_function(connection, session, object, script(PREPARE_FILL), deadline)?;
            if let Some(kind) = prepared
                .get("valueType")
                .and_then(Value::as_str)
                .and_then(InputValueKind::from_input_type)
            {
                let target = ValueTarget {
                    kind,
                    text,
                    backend_node_id: token.backend_node_id,
                };
                return assign_value(connection, session, object, target, deadline);
            }
            if let Some(input_type) = prepared.get("partOf").and_then(Value::as_str) {
                return Err(part_of_input(token.backend_node_id, input_type));
            }
            if prepared.get("editable").and_then(Value::as_bool) != Some(true) {
                let reason = prepared.get("reason").and_then(Value::as_str);
                return Err(not_editable(token.backend_node_id, reason));
            }
            if text.is_empty() {
                press_key(connection, session, "Backspace", deadline)?;
            } else {
                connection.call(
                    "Input.insertText",
                    json!({ "text": text }),
                    Some(&session.session_id),
                    deadline,
                )?;
            }
            let observed =
                call_function(connection, session, object, script(READ_FILLED), deadline)?
                    .as_str()
                    .map(str::to_owned);
            Ok(FillOutcome {
                verified: observed
                    .as_deref()
                    .is_some_and(|observed| observed.trim_end_matches('\n') == text),
                observed,
                assigned: None,
            })
        },
    )
}

fn script(function: &'static str) -> ElementCall {
    ElementCall {
        function,
        arguments: Vec::new(),
    }
}

fn not_editable(backend_node_id: i64, reason: Option<&str>) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("DOM node {backend_node_id} does not accept typed text"),
    )
    .with_platform_detail(reason.unwrap_or("not editable"))
    .with_details(json!({ "kind": "cdp_not_editable", "backend_node_id": backend_node_id }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;
