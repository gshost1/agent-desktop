use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::element_call::ElementCall;
use super::remote_element::call_on_element;
use crate::{CdpConnection, PageSession};

const SELECT_OPTION: &str = r"function(choice) {
  if (this.localName !== 'select') return { error: 'not_select' };
  const options = Array.from(this.options);
  const wanted = String(choice).trim();
  const match = options.find(o => o.label.trim() === wanted)
    || options.find(o => o.value === wanted)
    || options.find(o => o.text.trim() === wanted);
  if (!match) return { error: 'no_option', options: options.map(o => o.label.trim()) };
  if (match.disabled) return { error: 'disabled_option' };
  if (this.multiple) { match.selected = true; } else { this.selectedIndex = match.index; }
  const View = this.ownerDocument.defaultView;
  this.dispatchEvent(new View.Event('input', { bubbles: true }));
  this.dispatchEvent(new View.Event('change', { bubbles: true }));
  return { value: this.value, label: match.label.trim() };
}";

/// Selects the `<option>` of a `<select>` whose label, value or text is
/// `choice`, then fires `input` and `change` as a user's pick would. A
/// native select's popup cannot be driven with page input, so the value is
/// set directly. Returns the select's new value.
pub fn select_option(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    choice: &str,
    deadline: Deadline,
) -> Result<String, AdapterError> {
    let outcome = call_on_element(
        connection,
        session,
        backend_node_id,
        ElementCall {
            function: SELECT_OPTION,
            arguments: vec![json!(choice)],
        },
        deadline,
    )?;
    match outcome.get("error").and_then(Value::as_str) {
        None => Ok(outcome
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()),
        Some("not_select") => Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!("DOM node {backend_node_id} is not a <select>"),
        )
        .with_disposition(DeliverySemantics::not_delivered())),
        Some(reason) => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            format!("No selectable option matches '{choice}'"),
        )
        .with_details(json!({
            "kind": "cdp_option_unavailable",
            "reason": reason,
            "options": outcome.get("options").cloned().unwrap_or(Value::Null),
        }))
        .with_disposition(DeliverySemantics::not_delivered())),
    }
}

#[cfg(test)]
#[path = "select_option_tests.rs"]
mod tests;
