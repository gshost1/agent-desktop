use super::*;
use std::time::Duration;

fn surface(id: &str, kind: &str) -> SurfaceInfo {
    SurfaceInfo {
        id: id.into(),
        kind: kind.into(),
        title: Some(format!("{kind} title")),
        item_count: Some(3),
        unclassified: Vec::new(),
    }
}

fn incomplete(code: ErrorCode) -> AdapterError {
    AdapterError::new(code, "incomplete")
        .with_details(serde_json::json!({ "complete": false, "attribute": "AXChildren" }))
}

#[test]
fn shared_menu_discovery_preserves_other_surfaces_and_existing_menu_ids() {
    let window_surfaces = vec![
        surface("w-31", "sheet"),
        surface("w-30/popover/0", "popover"),
        surface("w-32", "alert"),
    ];
    for menus in [
        vec![],
        vec![surface("app/menubar/children/1/children/0", "menu")],
    ] {
        let mut expected = menus.clone();
        expected.extend(window_surfaces.clone());
        let actual = combine_surfaces(window_surfaces.clone(), Ok(menus)).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}

#[test]
fn an_unfinished_menu_scan_keeps_the_sheets_and_marks_menus_unread() {
    for code in [ErrorCode::AppUnresponsive, ErrorCode::Timeout] {
        let sheet = surface("w-31", "sheet");

        let surfaces = combine_surfaces(vec![sheet.clone()], Err(incomplete(code))).unwrap();

        assert_eq!(surfaces.len(), 2);
        assert_eq!(surfaces[0].id, sheet.id);
        assert_eq!(surfaces[1].id, "app/menubar");
        assert_eq!(surfaces[1].kind, "menubar");
        assert_eq!(surfaces[1].unclassified, ["menu"]);
    }
}

#[test]
fn an_unfinished_menu_scan_is_never_reported_as_an_empty_inventory() {
    let surfaces = combine_surfaces(Vec::new(), Err(incomplete(ErrorCode::Timeout))).unwrap();

    assert!(!surfaces.is_empty());
    assert!(surfaces.iter().all(|entry| entry.kind != "menu"));
    assert_eq!(surfaces[0].unclassified, ["menu"]);
}

#[test]
fn a_menu_scan_that_lost_the_process_or_permission_still_fails() {
    for error in [
        AdapterError::stale_ref_because("Application process instance changed during observation"),
        AdapterError::permission_denied(),
    ] {
        let code = error.code.clone();

        let failure = combine_surfaces(vec![surface("w-31", "sheet")], Err(error))
            .expect_err("terminal menu errors propagate");

        assert_eq!(failure.code, code);
    }
}

#[test]
fn expired_deadline_fails_before_accessibility_inventory() {
    let deadline = Instant::now() - Duration::from_millis(1);
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)
        .unwrap()
        .unwrap();
    let error =
        list_surfaces_for_process(identity, deadline).expect_err("expired surface inventory");
    assert_eq!(error.code.as_str(), "TIMEOUT");
}

fn node_cap() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({ "kind": "surface_search_limit", "complete": false }))
}

#[test]
fn a_capped_menu_search_with_no_menu_window_on_screen_reads_as_no_menu() {
    let windows = vec![surface("w-31", "sheet")];
    let settled =
        finish_surface_inventory(windows.clone(), Err(node_cap()), || Ok(Vec::new())).unwrap();

    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].id, windows[0].id);
}

#[test]
fn a_capped_menu_search_with_a_menu_window_on_screen_stays_unfinished() {
    for menu_windows in [
        Ok(vec![88]),
        Err(AdapterError::internal("window list unavailable")),
    ] {
        let surfaces =
            finish_surface_inventory(Vec::new(), Err(node_cap()), || menu_windows).unwrap();
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].unclassified, ["menu"]);
    }
}

#[test]
fn a_timed_out_menu_search_is_not_settled_by_the_window_server() {
    let settled = settle_by_window_server(Err(incomplete(ErrorCode::Timeout)), || Ok(Vec::new()));

    assert_eq!(
        settled.expect_err("timeout stays unfinished").code,
        ErrorCode::Timeout
    );
}
