//! The observable effect of a key sent while a hosted view holds the
//! application's focus. The application's focused element, its state and the
//! window-server windows of the application and the focus owner are read
//! before and after delivery; a key that changed none of them did nothing
//! the caller can rely on.

use agent_desktop_core::{ActionResult, AdapterError, Deadline, DeliverySemantics, ErrorCode};
use core_foundation::base::{CFType, TCFType};
use rustc_hash::FxHashSet;
use std::time::{Duration, Instant};

use super::cg_window::WindowRecord;
use super::hosted_focus_read::HostedFocus;
use crate::tree::AXElement;

const STATE_ATTRIBUTES: [&str; 7] = [
    "AXRole",
    "AXValue",
    "AXSelectedTextRange",
    "AXSelectedRows",
    "AXSelectedChildren",
    "AXPosition",
    "AXSize",
];
const SETTLE_LIMIT: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub(crate) struct FocusSignature {
    focused: Option<AXElement>,
    state: Option<CFType>,
    windows: Vec<WindowRecord>,
}

pub(crate) struct EffectWatch {
    pids: FxHashSet<i32>,
    before: FocusSignature,
}

impl FocusSignature {
    pub(crate) fn differs_from(&self, other: &Self) -> bool {
        let focus_moved = match (&self.focused, &other.focused) {
            (Some(left), Some(right)) => !crate::tree::same_element(left, right),
            (None, None) => false,
            _ => true,
        };
        focus_moved
            || !same_value(self.state.as_ref(), other.state.as_ref())
            || self.windows != other.windows
    }
}

pub(crate) fn watch(
    focus: &HostedFocus,
    app: &AXElement,
    deadline: Deadline,
) -> Result<Option<EffectWatch>, AdapterError> {
    let Some(pids) = focus.watched_pids() else {
        return Ok(None);
    };
    let before = read_signature(app, &pids, deadline)?;
    Ok(Some(EffectWatch { pids, before }))
}

/// Settles a delivered press: a change in the watched focus or windows makes
/// the result verified, no change within the settle limit is an error whose
/// disposition keeps the delivery uncertain.
pub(crate) fn finish_press(
    focus: &HostedFocus,
    app: &AXElement,
    watch: Option<EffectWatch>,
    result: ActionResult,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let Some(watch) = watch else {
        return Ok(with_target(result, focus.details()));
    };
    let observed = wait_for_change(app, &watch, deadline)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    tracing::debug!(observed, "keyboard: hosted focus effect");
    let target = focus.details().map(|mut target| {
        target["effect"] = serde_json::json!(if observed {
            "observed"
        } else {
            "none_observed"
        });
        target
    });
    if observed {
        return Ok(with_target(result.with_verified_delivery(), target));
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        if focus.is_hosted_view() {
            "The key went to the process that owns the focused panel, but neither the focus nor any window changed"
        } else {
            "The focus is in a view another process owns, and the key sent to the app changed neither the focus nor any window"
        },
    )
    .with_details(serde_json::json!({ "keyboard_target": target }))
    .with_disposition(DeliverySemantics::delivered_unverified())
    .with_suggestion(
        "Do not repeat the key: read the panel again, then use its controls by ref: set-value on its text fields, select on its pop-up buttons and click on its default button",
    ))
}

fn with_target(mut result: ActionResult, target: Option<serde_json::Value>) -> ActionResult {
    if let Some(target) = target {
        result.details = Some(merge_detail(result.details, "keyboard_target", target));
    }
    result
}

/// Sets `key` in a details object and keeps every other entry, so a report
/// added late, such as `keyboard_target`, never erases `menu_shortcut` or
/// `modal_key`. A details value that is not an object is kept under `report`.
pub(crate) fn merge_detail(
    details: Option<serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) -> serde_json::Value {
    let mut merged = match details {
        Some(serde_json::Value::Object(map)) => map,
        Some(other) => [("report".to_string(), other)].into_iter().collect(),
        None => serde_json::Map::new(),
    };
    merged.insert(key.to_string(), value);
    serde_json::Value::Object(merged)
}

fn wait_for_change(
    app: &AXElement,
    watch: &EffectWatch,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let settle = deadline.capped(SETTLE_LIMIT);
    let limit = Instant::now() + settle.remaining();
    loop {
        if read_signature(app, &watch.pids, deadline)?.differs_from(&watch.before) {
            return Ok(true);
        }
        if Instant::now() >= limit {
            return Ok(false);
        }
        let pause = deadline.remaining_slice(POLL_INTERVAL)?;
        std::thread::sleep(pause.min(POLL_INTERVAL));
    }
}

fn read_signature(
    app: &AXElement,
    pids: &FxHashSet<i32>,
    deadline: Deadline,
) -> Result<FocusSignature, AdapterError> {
    use super::cg_window::{WindowRecordScope, records_from_dictionaries, window_dictionaries};

    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let focused = crate::tree::surface_read::element(app, "AXFocusedUIElement", instant);
    deadline.ensure_remaining()?;
    let focused = focused.ok().flatten();
    let state = focused
        .as_ref()
        .and_then(|element| read_state(element, deadline));
    deadline.ensure_remaining()?;
    let windows = records_from_dictionaries(window_dictionaries()?, WindowRecordScope::Pids(pids))?;
    Ok(FocusSignature {
        focused,
        state,
        windows,
    })
}

fn read_state(element: &AXElement, deadline: Deadline) -> Option<CFType> {
    use core_foundation::array::CFArray;
    use core_foundation::string::CFString;

    let names: Vec<CFString> = STATE_ATTRIBUTES
        .iter()
        .map(|name| CFString::new(name))
        .collect();
    let attributes = CFArray::from_CFTypes(&names);
    let (error, values) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        element,
        attributes.as_concrete_TypeRef(),
        deadline,
    );
    if values.is_null() {
        return None;
    }
    let values = unsafe { CFType::wrap_under_create_rule(values) };
    (error == accessibility_sys::kAXErrorSuccess).then_some(values)
}

fn same_value(left: Option<&CFType>, right: Option<&CFType>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "hosted_focus_effect_tests.rs"]
mod tests;
