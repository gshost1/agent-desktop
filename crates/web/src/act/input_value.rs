use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::assigned_input::AssignedInput;
use super::element_call::ElementCall;
use super::fill_outcome::FillOutcome;
use super::input_value_kind::InputValueKind;
use super::remote_element::call_function;
use super::value_target::ValueTarget;
use crate::{CdpConnection, PageSession};

/// Assigns the value through the prototype's `value` setter, so a page
/// framework that tracks the value on the element itself still sees a
/// change, then fires `input` and `change` as a user edit would and reads
/// the value back. A value the input sanitizes to empty is put back and
/// reported as rejected before the input is focused, so a refusal fires no
/// event and moves no focus.
const ASSIGN_VALUE: &str = r"function(value) {
  const before = this.value;
  if (before === value) return { status: 'unchanged', observed: before };
  let owner = Object.getPrototypeOf(this);
  while (owner && !Object.getOwnPropertyDescriptor(owner, 'value')) owner = Object.getPrototypeOf(owner);
  const descriptor = owner ? Object.getOwnPropertyDescriptor(owner, 'value') : null;
  const assign = (next) => { if (descriptor && descriptor.set) descriptor.set.call(this, next); else this.value = next; };
  assign(value);
  if (value !== '' && this.value === '') {
    assign(before);
    return { status: 'rejected', observed: this.value };
  }
  this.focus();
  this.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  this.dispatchEvent(new Event('change', { bubbles: true }));
  return { status: 'assigned', observed: this.value };
}";

/// Sets an input that takes its value whole to `target.text`. Text that is
/// not a value of the input's type, or not in the input's own format, is
/// `INVALID_ARGS` before the page is touched. A value the input rejects is
/// `ACTION_FAILED` and leaves the old value. A read-back that differs from
/// the value is `ACTION_FAILED` with the observed value; a read-back that
/// cannot be read leaves the outcome unverified.
pub(crate) fn assign_value(
    connection: &mut CdpConnection,
    session: &PageSession,
    object_id: &str,
    target: ValueTarget<'_>,
    deadline: Deadline,
) -> Result<FillOutcome, AdapterError> {
    let kind = target.kind;
    let value = match kind.canonical(target.text) {
        Some(value) if value == target.text => value,
        Some(value) => return Err(not_in_format(kind, target.text, &value)),
        None => return Err(malformed(kind, target.text)),
    };
    let reply = call_function(
        connection,
        session,
        object_id,
        ElementCall {
            function: ASSIGN_VALUE,
            arguments: vec![json!(value)],
        },
        deadline,
    )?;
    let observed = reply
        .get("observed")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let assigned = |unchanged| AssignedInput {
        input_type: kind.input_type().to_owned(),
        unchanged,
    };
    match reply.get("status").and_then(Value::as_str) {
        Some("unchanged") => Ok(FillOutcome {
            observed,
            verified: true,
            assigned: Some(assigned(true)),
        }),
        Some("rejected") => Err(rejected(kind, &value, target.backend_node_id)),
        _ => match observed {
            Some(observed) if observed != value => Err(adjusted(kind, &value, &observed)),
            observed => Ok(FillOutcome {
                verified: observed.is_some(),
                observed,
                assigned: Some(assigned(false)),
            }),
        },
    }
}

/// The refusal for a node that is one part of an input taking its value
/// whole, such as the hours field of a time input: its value is the
/// input's, so it is set on the input.
pub(crate) fn part_of_input(backend_node_id: i64, input_type: &str) -> AdapterError {
    let expected = InputValueKind::from_input_type(input_type).map_or_else(String::new, |kind| {
        format!(
            " in its format, {}, for example {}",
            kind.format(),
            kind.example()
        )
    });
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("DOM node {backend_node_id} is one part of a {input_type} input and has no value of its own"),
    )
    .with_suggestion(format!(
        "Set the whole value with set-value on the {input_type} input's own ref{expected}"
    ))
    .with_details(json!({
        "kind": "cdp_input_part",
        "backend_node_id": backend_node_id,
        "input_type": input_type,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn malformed(kind: InputValueKind, text: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!(
            "'{text}' is not a value a {} input takes",
            kind.input_type()
        ),
    )
    .with_suggestion(takes(kind))
    .with_details(format_details(kind, "cdp_value_malformed", None))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn not_in_format(kind: InputValueKind, text: &str, value: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!(
            "'{text}' is not in the {} input's own format",
            kind.input_type()
        ),
    )
    .with_suggestion(format!("Pass '{value}'. {}", takes(kind)))
    .with_details(format_details(kind, "cdp_value_not_canonical", Some(value)))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn rejected(kind: InputValueKind, value: &str, backend_node_id: i64) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "The {} input rejected '{value}' and kept its old value",
            kind.input_type()
        ),
    )
    .with_platform_detail(format!(
        "DOM node {backend_node_id} sanitized the value to empty"
    ))
    .with_suggestion(format!(
        "{} Use a value that exists, such as a real calendar date.",
        takes(kind)
    ))
    .with_details(format_details(kind, "cdp_value_rejected", Some(value)))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn adjusted(kind: InputValueKind, value: &str, observed: &str) -> AdapterError {
    let mut details = format_details(kind, "cdp_value_adjusted", Some(value));
    details["observed"] = json!(observed);
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "The {} input holds '{observed}' after set-value '{value}'",
            kind.input_type()
        ),
    )
    .with_suggestion(
        "The input or the page changed the value, for example to fit min, max or step. \
         Read it with 'get @ref --property value' before you set it again.",
    )
    .with_details(details)
    .with_disposition(DeliverySemantics::delivered_unverified())
}

fn takes(kind: InputValueKind) -> String {
    format!(
        "A {} input takes {}, for example {}.",
        kind.input_type(),
        kind.format(),
        kind.example()
    )
}

fn format_details(kind: InputValueKind, error_kind: &str, value: Option<&str>) -> Value {
    let mut details = json!({
        "kind": error_kind,
        "input_type": kind.input_type(),
        "expected_format": kind.format(),
    });
    if let Some(value) = value {
        details["value"] = json!(value);
    }
    details
}

#[cfg(test)]
#[path = "input_value_tests.rs"]
mod tests;
