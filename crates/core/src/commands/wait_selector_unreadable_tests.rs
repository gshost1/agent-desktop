use super::*;

fn failing(code: ErrorCode) -> TwoWindowApp {
    let mut app = TwoWindowApp::new(vec![
        pane("w-1", OURS, true, Field::Nowhere),
        pane("w-2", OURS, false, Field::Nowhere),
    ]);
    app.failure = Some(code);
    app
}

#[test]
fn a_revoked_permission_ends_a_menu_bar_wait_as_permission_denied() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = failing(ErrorCode::PermDenied);
    let entry = menubar_entry();
    let baseline = baseline(&app, &entry);

    let error = adapter_error(click_and_wait(&app, &entry, baseline.as_ref()));

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    assert_eq!(error.details.unwrap()["after_action"]["action"], "click");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn an_unresponsive_owner_ends_the_wait_after_the_ref_window_closed() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = failing(ErrorCode::AppUnresponsive);
    let entry = window_entry("w-closed");

    let error = adapter_error(click_and_wait(&app, &entry, None));

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    assert_eq!(error.details.unwrap()["kind"], "wait_owner_unresponsive");
    assert!(clock.elapsed() >= ms(5_000), "{:?}", clock.elapsed());
    assert!(clock.elapsed() < ms(6_000), "{:?}", clock.elapsed());
}
