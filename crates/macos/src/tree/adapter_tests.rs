use std::time::Duration;

use agent_desktop_core::ObservationOps;

#[test]
fn a_closed_menu_wait_holds_three_hundred_milliseconds_on_macos() {
    let adapter = crate::adapter::MacOSAdapter::new();
    assert_eq!(adapter.menu_opener_grace(), Duration::from_millis(300));
    assert!(adapter.menu_opener_grace() < agent_desktop_core::MENU_OPENER_GRACE);
}
