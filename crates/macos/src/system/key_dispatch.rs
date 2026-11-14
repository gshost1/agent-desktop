use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionPolicy, KeyCombo, ProcessIdentity, StepMechanism,
};
use std::time::{Duration, Instant};

use crate::system::menu_shortcut::{
    MENU_SHORTCUT_STEP, menu_shortcut_outcome::MenuShortcutOutcome, press_menu_shortcut,
};
use crate::system::window::hosted_focus_effect::EffectWatch;
use crate::tree::AXElement;

#[path = "key_dispatch_modal.rs"]
mod modal;

#[path = "key_dispatch_modal_route.rs"]
mod modal_route;

#[path = "key_dispatch_target.rs"]
mod target;

use target::{KeyTarget, host_key_action};

pub(crate) fn press_for_app_impl(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    press_for_app(process, combo, policy, deadline)
        .map_err(|error| crate::actions::DeliveryTracker::default().annotate(error))
}

fn press_for_app(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    tracing::debug!(pid = process.pid.get(), "system: press key for app");
    let identity = crate::system::process::process_identity::require_core(&process)?;
    let pid = identity.pid();
    let app = crate::tree::element_for_pid(pid);
    let hosted = crate::system::window::hosted_focus_read::resolve(&app, pid, deadline)?;
    let target = KeyTarget {
        process: &process,
        app: &app,
        hosted: hosted.as_ref(),
    };
    let host_owns_keys = target.host_owns_keys();

    let menu = if host_owns_keys && uses_menu_shortcut(combo, policy) {
        press_menu_shortcut(&process, &app, combo, deadline)?
    } else {
        MenuShortcutOutcome::NotApplicable
    };
    if matches!(menu, MenuShortcutOutcome::Pressed(_)) {
        return post_delivery_process_result(&process, menu_shortcut_report(&menu));
    }
    if let Some(action) = host_key_action(host_owns_keys, combo) {
        if let Some(modal) = modal::ModalWatch::capture(&app, action, deadline)? {
            return modal_route::press_on_modal(&target, (combo, policy), &modal, deadline)
                .map(|result| target.report(result))
                .map_err(|error| target.annotate(error));
        }
        if try_simple_key_action(&app, action, deadline)? {
            return post_delivery_process_result(&process, semantic_step(action).into());
        }
    }
    let (step, watch) = send_physical_key(&target, (combo, policy), deadline, true)?;
    let mut report = menu_shortcut_report(&menu);
    report.steps.push(step);
    let result = post_delivery_process_result(&process, report)?;
    match &hosted {
        Some(focus) => crate::system::window::hosted_focus_effect::finish_press(
            focus, &app, watch, result, deadline,
        ),
        None => Ok(result),
    }
}

/// Sends the real key to the pid the target picks. `watch_effect` reads the
/// hosted focus state first, for the route that verifies by its change.
fn send_physical_key(
    target: &KeyTarget<'_>,
    (combo, policy): (&KeyCombo, InteractionPolicy),
    deadline: Deadline,
    watch_effect: bool,
) -> Result<(ActionStep, Option<EffectWatch>), AdapterError> {
    use crate::system::process::process_identity::require_core;

    let pid = require_core(target.process)?.pid();
    crate::input::keyboard::keyboard_map::key_name_to_code(&combo.key)?;
    if policy.allow_focus_steal {
        require_core(target.process)?;
        crate::system::focus::verify_app_focused(pid, deadline)?;
    }
    require_core(target.process)?;
    require_focused_element(target.app, policy, deadline)?;
    require_core(target.process)?;
    let delivery_pid = target.delivery_pid(deadline)?;
    let watch = match target.hosted {
        Some(focus) if watch_effect => {
            crate::system::window::hosted_focus_effect::watch(focus, target.app, deadline)?
        }
        _ => None,
    };
    crate::input::keyboard::synthesize_key(combo, Some(delivery_pid), deadline)?;
    let step = ActionStep::succeeded("CGEventPostToPid")
        .with_mechanism(StepMechanism::PhysicalSynthetic)
        .with_verified(false);
    Ok((step, watch))
}

/// What the press response says about the menu-bar lookup: the menu item it
/// pressed, or why the chord went to the keyboard after the lookup applied.
struct DeliveryReport {
    steps: Vec<ActionStep>,
    details: Option<serde_json::Value>,
}

impl From<ActionStep> for DeliveryReport {
    fn from(step: ActionStep) -> Self {
        Self {
            steps: vec![step],
            details: None,
        }
    }
}

fn menu_shortcut_report(outcome: &MenuShortcutOutcome) -> DeliveryReport {
    let (step, details) = match outcome {
        MenuShortcutOutcome::Pressed(details) => (semantic_step(MENU_SHORTCUT_STEP), details),
        MenuShortcutOutcome::Skipped(details) => (
            ActionStep::skipped(MENU_SHORTCUT_STEP).with_mechanism(StepMechanism::SemanticApi),
            details,
        ),
        MenuShortcutOutcome::NotApplicable => {
            return DeliveryReport {
                steps: Vec::new(),
                details: None,
            };
        }
    };
    DeliveryReport {
        steps: vec![step],
        details: Some(serde_json::json!({ "menu_shortcut": details })),
    }
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

fn post_delivery_process_result(
    process: &ProcessIdentity,
    report: DeliveryReport,
) -> Result<ActionResult, AdapterError> {
    crate::system::process::process_identity::require_core(process)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    let result = ActionResult::delivered_unverified("press_key").with_steps(report.steps);
    Ok(match report.details {
        Some(details) => result.with_details(details),
        None => result,
    })
}

fn try_simple_key_action(
    app: &AXElement,
    action: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let Some(focused) = focused_element(app, deadline)? else {
        return Ok(false);
    };
    if crate::actions::ax_helpers::advertises_action(&focused, action, deadline) != Some(true) {
        return Ok(false);
    }
    crate::actions::ax_helpers::try_ax_action_or_err(&focused, action, deadline)
}

/// A command chord tries the menu bar under either policy. Headless, the
/// keystroke lands only while the app is frontmost, which a background app is
/// not; headed, core has already focused the window, and the menu item runs the
/// same command the key equivalent would.
fn uses_menu_shortcut(combo: &KeyCombo, _policy: InteractionPolicy) -> bool {
    combo
        .modifiers
        .contains(&agent_desktop_core::Modifier::Meta)
}

fn simple_key_action(combo: &KeyCombo) -> Option<&'static str> {
    if !combo.modifiers.is_empty() {
        return None;
    }
    match combo.key.as_str() {
        "return" | "enter" => Some("AXConfirm"),
        "escape" | "esc" => Some("AXCancel"),
        _ => None,
    }
}

fn no_focused_element_error(policy: InteractionPolicy) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Application has no verified focused element for keyboard delivery",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
    .with_suggestion(if policy.allow_focus_steal {
        "Ensure the target field has keyboard focus before retrying, or run the menu command without keys: find the item with 'snapshot --app <app> --surface menubar' and click its ref."
    } else {
        "Retry with --headed so the app can take focus, or run the menu command without \
         keys: find the item with 'snapshot --app <app> --surface menubar' and click its ref."
    })
}

fn require_focused_element(
    app: &AXElement,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<AXElement, AdapterError> {
    let local_deadline = local_deadline(deadline, Duration::from_millis(500))?;
    loop {
        if let Some(focused) = focused_element(app, deadline)? {
            return Ok(focused);
        }
        deadline.ensure_remaining()?;
        if Instant::now() >= local_deadline {
            return Err(no_focused_element_error(policy));
        }
        let pause = deadline.remaining_slice(Duration::from_millis(5))?;
        std::thread::sleep(pause.min(Duration::from_millis(5)));
    }
}

fn focused_element(app: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    element_attribute(app, "AXFocusedUIElement", deadline)
}

fn element_attribute(
    element: &AXElement,
    attribute: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let result =
        crate::tree::properties::attributes::copy_element_attr_result(element, attribute, deadline);
    deadline.ensure_remaining()?;
    result.map_err(|error| read_error(attribute, error))
}

fn local_deadline(deadline: Deadline, maximum: Duration) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining_slice(maximum)?;
    Instant::now()
        .checked_add(remaining)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Accessibility read failed for {attribute} during key dispatch"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(test)]
#[path = "key_dispatch_tests.rs"]
mod tests;
