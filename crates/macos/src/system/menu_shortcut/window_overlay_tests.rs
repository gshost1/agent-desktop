use super::*;

#[test]
fn sheets_dialogs_alerts_and_popovers_hold_the_chord() {
    assert!(is_overlay_role(Some("AXSheet"), None));
    assert!(is_overlay_role(Some("AXPopover"), None));
    assert!(is_overlay_role(Some("AXWindow"), Some("AXDialog")));
    assert!(is_overlay_role(Some("AXWindow"), Some("AXSystemDialog")));
    assert!(is_overlay_role(Some("AXGroup"), Some("AXAlert")));
}

#[test]
fn ordinary_windows_and_content_leave_the_menu_bar_reachable() {
    assert!(!is_overlay_role(Some("AXWindow"), Some("AXStandardWindow")));
    assert!(!is_overlay_role(Some("AXWindow"), Some("AXFloatingWindow")));
    assert!(!is_overlay_role(Some("AXToolbar"), None));
    assert!(!is_overlay_role(None, None));
}
