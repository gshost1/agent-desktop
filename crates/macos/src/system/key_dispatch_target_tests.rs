use super::*;

fn key(name: &str) -> KeyCombo {
    KeyCombo {
        key: name.into(),
        modifiers: Vec::new(),
    }
}

#[test]
fn a_proven_hosted_view_takes_confirm_and_cancel_off_the_sheet_route() {
    for name in ["return", "enter", "escape", "esc"] {
        assert_eq!(host_key_action(false, &key(name)), None, "{name}");
    }
}

#[test]
fn a_host_that_owns_its_keys_keeps_the_sheet_route_for_confirm_and_cancel() {
    assert_eq!(host_key_action(true, &key("return")), Some("AXConfirm"));
    assert_eq!(host_key_action(true, &key("enter")), Some("AXConfirm"));
    assert_eq!(host_key_action(true, &key("escape")), Some("AXCancel"));
    assert_eq!(host_key_action(true, &key("esc")), Some("AXCancel"));
    assert_eq!(host_key_action(true, &key("a")), None);
}

#[test]
fn a_target_without_hosted_focus_owns_its_keys_and_adds_no_report() {
    let process = ProcessIdentity::new(u32::MAX, "invalid");
    let app = crate::tree::element_for_pid(-1);
    let target = KeyTarget {
        process: &process,
        app: &app,
        hosted: None,
    };
    assert!(target.host_owns_keys());
    let details = serde_json::json!({ "modal_key": { "effect": "observed" } });
    let result = target
        .report(ActionResult::delivered_unverified("press_key").with_details(details.clone()));
    assert_eq!(result.details, Some(details));
}
