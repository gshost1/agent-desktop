use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::ref_action::*;
use crate::{
    ErrorCode, actions::action::Action, refs::RefEntry, tree::element_state::ElementState,
};

/// A list row with no semantic action, read live on screen while its refmap
/// entry carries the same rectangle measured inside its window.
struct RowWithoutAction;

fn live_on_screen() -> crate::Rect {
    crate::Rect {
        x: 471.0,
        y: 499.0,
        width: 230.0,
        height: 43.0,
    }
}

fn in_window() -> crate::Rect {
    crate::Rect {
        x: 1.0,
        y: 279.0,
        width: 230.0,
        height: 43.0,
    }
}

impl ObservationOps for RowWithoutAction {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::LiveElement, AdapterError> {
        Ok(crate::LiveElement {
            identity: crate::adapter::live_identity("Displays"),
            state: ElementState {
                role: "listitem".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(live_on_screen()),
            available_actions: Vec::new(),
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        Ok(Some(live_on_screen()))
    }
    crate::adapter::saved_ref_scope!("listitem", Some("Displays"));
}

impl ActionOps for RowWithoutAction {}
impl InputOps for RowWithoutAction {}
impl SystemOps for RowWithoutAction {
    crate::adapter::guarded_interaction_lease!();
}

fn entry(space: Option<crate::BoundsSpace>, bounds: crate::Rect) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("listitem", Some("Displays"));
    entry.geometry = crate::RefGeometry {
        bounds_space: space,
        bounds: Some(bounds),
        bounds_hash: bounds.bounds_hash(),
    };
    entry
}

fn stable_check(error: &AdapterError) -> Option<String> {
    error
        .details
        .as_ref()?
        .get("checks")?
        .as_array()?
        .iter()
        .find(|check| check["check"] == "stable")
        .map(|check| check["status"].as_str().unwrap_or_default().to_owned())
}

fn click(entry: &RefEntry) -> AdapterError {
    let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(400));
    execute_entry(&RowWithoutAction, entry, request).unwrap_err()
}

#[test]
fn a_window_space_ref_is_refused_for_its_real_reason_alone() {
    let error = click(&entry(Some(crate::BoundsSpace::Window), in_window()));

    assert_ne!(error.code, ErrorCode::Timeout, "{error:?}");
    assert!(!error.message.contains("stable"), "{}", error.message);
    assert_ne!(stable_check(&error).as_deref(), Some("fail"), "{error:?}");
    assert!(
        error.message.contains("supported_action"),
        "{}",
        error.message
    );
}

#[test]
fn a_screen_space_ref_that_moved_still_fails_the_stable_check() {
    let error = click(&entry(None, in_window()));

    assert!(error.message.contains("stable"), "{}", error.message);
    assert_eq!(stable_check(&error).as_deref(), Some("fail"), "{error:?}");
}
