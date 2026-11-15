//! Turns a report that is not actionable into the refusal the caller sees, with
//! a suggestion that names the next step for the check that failed.

use std::time::Duration;

use serde_json::{Value, json};

use super::{
    check::ActionabilityCheck, evidence::ActionabilityEvidence, report::ActionabilityReport,
    status::ActionabilityStatus,
};
use crate::{Action, AdapterError, ErrorCode, actions::action_request::ActionRequest};

/// How long a ref action on the default budget keeps polling a target whose
/// live enabled state is false before it ends with a final refusal. Measured
/// on GTK3 and GTK4 under AT-SPI: a button enabled by the caller's own typing
/// is enabled at the first poll, and one a 1 s timer enables is seen enabled
/// 1,080 to 1,200 ms (GTK3) and 1,420 to 1,630 ms (GTK4) after the click
/// starts, so 2 s keeps that case and cuts a control that stays disabled from
/// the 5 s default to about 2 s. Other toolkits were not measured; a control
/// enabled later is reached with `wait --element <ref> --predicate enabled` or
/// a `--timeout-ms` the caller chooses.
pub(crate) const DISABLED_SETTLE: Duration = Duration::from_millis(2000);

const DISABLED_SUGGESTION: &str = "The element is disabled. Do what enables it first, such as \
     filling a required field or selecting an item, then act on it again. If it becomes enabled \
     on its own, wait for that with wait --element <ref> --predicate enabled --timeout <ms>, \
     then act.";

const WAITABLE_SUGGESTION: &str = "Wait for the target to become actionable, refresh the \
     snapshot, or use an explicit physical/focus command if intended.";

const NOT_SATISFIABLE_SUGGESTION: &str = "Waiting will not help: this element cannot satisfy the \
     action as targeted. Target an element that advertises the action (check available_actions in \
     a fresh snapshot). For a dropdown without select support, open it with click, inspect its \
     choices, and use explicit keyboard input.";

pub(super) fn finish(
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    report: ActionabilityReport,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    if report.actionable {
        return Ok(report);
    }
    let code = report.terminal_code().unwrap_or(ErrorCode::ActionFailed);
    let sole = report.sole_blocking_check();
    let disabled = sole.is_some_and(is_disabled);
    let mut details = report_details(evidence, &report);
    let elapsed = deadline.elapsed();
    let message = if disabled && disabled_is_final(request, elapsed) {
        mark_final_disabled(&mut details, elapsed);
        format!(
            "Target is disabled: its live enabled state is still false after {} ms",
            elapsed.as_millis()
        )
    } else {
        format!("Target is not actionable: {}", report.failure_reasons())
    };
    let suggestion = if disabled {
        DISABLED_SUGGESTION.to_string()
    } else if let Some(hint) = fallback_denied_suggestion(&report, request) {
        hint.to_string()
    } else {
        suggestion(evidence, request, &report, &code).to_string()
    };
    Err(AdapterError::new(code, message)
        .with_details(details)
        .with_suggestion(suggestion)
        .with_disposition(crate::DeliverySemantics::not_delivered()))
}

fn report_details(evidence: &ActionabilityEvidence, report: &ActionabilityReport) -> Value {
    let mut details = json!(report);
    if let Some(object) = details.as_object_mut()
        && let Some(bounds_hash) = evidence.bounds.and_then(|bounds| bounds.bounds_hash())
    {
        object.insert("observed_bounds_hash".into(), json!(bounds_hash));
    }
    details
}

fn is_disabled(check: &ActionabilityCheck) -> bool {
    check.check == "enabled" && check.status == ActionabilityStatus::Fail
}

/// Only a ref action polling on its command's default budget is cut short: a
/// caller that set its own budget asked to wait for the target, and a request
/// with no budget at all is not a polling ref action.
fn disabled_is_final(request: &ActionRequest, elapsed: Duration) -> bool {
    request.on_default_budget() && elapsed >= DISABLED_SETTLE
}

fn mark_final_disabled(details: &mut Value, elapsed: Duration) {
    if let Some(object) = details.as_object_mut() {
        object.insert("kind".into(), json!("target_disabled"));
        object.insert("retryable".into(), json!(false));
        object.insert(
            "elapsed_ms".into(),
            json!(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
        );
    }
}

/// The way out when the element advertises no semantic action for the request
/// and the policy keeps the command from using the pointer or keyboard focus
/// instead: the same command with `--headed`. The denial is terminal, so a
/// waitable gap reported beside it does not change the answer; a disabled
/// target or another terminal refusal, such as a target that is not editable,
/// does, because headed input cannot fix either. A headed request never gets
/// here: its policy permits the pointer and focus fallbacks this check denies.
/// Switching to headed input is the user's choice, so the text makes it
/// conditional on their permission.
fn fallback_denied_suggestion(
    report: &ActionabilityReport,
    request: &ActionRequest,
) -> Option<&'static str> {
    let denied = report.checks.iter().any(|check| {
        check.check == "supported_action"
            && check.status == ActionabilityStatus::Fail
            && check.terminal_code == Some(ErrorCode::PolicyDenied)
    });
    let other_terminal = report.checks.iter().any(|check| {
        check.check != "supported_action"
            && check.status == ActionabilityStatus::Fail
            && check.terminal_code.is_some()
    });
    if !denied || other_terminal || report.checks.iter().any(is_disabled) {
        return None;
    }
    Some(match request.action {
        Action::Click => {
            "This element has no semantic click action, so only the pointer can click it. If the \
             user allows physical input for this task, run the command with --headed; it moves \
             the pointer and may raise the window. Where this session has no headed pointer \
             input, the headed command refuses and says why."
        }
        Action::RightClick => {
            "This element has no semantic right-click action, so only the pointer can right-click \
             it. If the user allows physical input for this task, run the command with --headed; \
             it moves the pointer and may raise the window. Where this session has no headed \
             pointer input, the headed command refuses and says why."
        }
        Action::DoubleClick | Action::TripleClick => {
            "This element has no semantic action for this gesture, so only the pointer can send \
             it. If the user allows physical input for this task, run the command with --headed; \
             it moves the pointer and may raise the window. Where this session has no headed \
             pointer input, the headed command refuses and says why."
        }
        _ if request.action.may_use_focus_fallback() => {
            "This element has no semantic action for this input, so it has to take keyboard focus \
             first. If the user allows physical input for this task, run the command with \
             --headed to focus it and send the keys. Where this session has no headed input, the \
             headed command refuses and says why."
        }
        _ => return None,
    })
}

fn suggestion(
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    report: &ActionabilityReport,
    code: &ErrorCode,
) -> &'static str {
    let not_supported = *code == ErrorCode::ActionNotSupported;
    let editable_failed = report
        .checks
        .iter()
        .any(|check| check.check == "editable" && check.status == ActionabilityStatus::Fail);
    if *code == ErrorCode::ActionFailed {
        WAITABLE_SUGGESTION
    } else if evidence.state.role == "cell" && editable_failed {
        "This cell is not directly editable. Activate it, take a fresh snapshot, and target the text editor if one appears."
    } else if evidence.state.role == "incrementor" && editable_failed {
        "This incrementor has no value of its own to set. When it is one part of a date or time field, set the whole value with set-value on the field that contains it, the parent that advertises SetValue (on the web surface, a datefield or timefield)."
    } else if not_supported && matches!(request.action, Action::SetFocus) {
        "Choose an element advertising SetFocus in a fresh snapshot. To focus the owning window instead, use focus-window."
    } else if not_supported
        && matches!(request.action, Action::SetValue(_) | Action::Clear)
        && matches!(evidence.state.role.as_str(), "textfield" | "combobox")
    {
        if request.policy.is_headed() {
            "This field has no semantic text-write action. Use 'type' in the current headed mode for physical keyboard input; select the existing text first if you intend to replace it. If typing refuses because the field cannot be focused, use clipboard-set and the app's paste shortcut, then verify the field."
        } else {
            "This field has no semantic text-write action. Use '--headed type' for physical keyboard input; select the existing text first if you intend to replace it. If typing refuses because the field cannot be focused, use clipboard-set and the app's paste shortcut, then verify the field."
        }
    } else {
        NOT_SATISFIABLE_SUGGESTION
    }
}

#[cfg(test)]
#[path = "refusal_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "refusal_auto_wait_tests.rs"]
mod auto_wait_tests;
