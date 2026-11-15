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

fn after_the_action_opens(app: &mut TwoWindowApp, opened: Pane) -> Option<WindowBaseline> {
    let baseline = baseline(app, &window_entry("w-1"));
    app.panes.push(opened);
    baseline
}

#[test]
fn an_unreadable_window_does_not_hide_a_sheet_that_holds_the_selector() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut app = TwoWindowApp::new(vec![pane(
        "w-1",
        OURS,
        true,
        Field::InSheet(Duration::ZERO),
    )]);
    let mut opened = pane("w-2", OURS, false, Field::Nowhere);
    opened.fails = Some(ErrorCode::PermDenied);
    let baseline = after_the_action_opens(&mut app, opened);

    let result = click_and_wait(&app, &window_entry("w-1"), baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-1");
    assert_eq!(result["matched_selector"], ":Width");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn an_unresponsive_ref_window_does_not_hide_a_window_the_action_opened() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut own = pane("w-1", OURS, true, Field::Nowhere);
    own.fails = Some(ErrorCode::AppUnresponsive);
    let mut app = TwoWindowApp::new(vec![own]);
    let baseline =
        after_the_action_opens(&mut app, pane("w-2", OURS, false, Field::InWindow(ms(300))));

    let result = click_and_wait(&app, &window_entry("w-1"), baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-2");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn overlay_probes_stop_once_the_wait_budget_is_spent() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let mut app = TwoWindowApp::new(
        ["w-1", "w-2", "w-3", "w-4"]
            .into_iter()
            .map(|id| pane(id, OURS, id == "w-1", Field::Nowhere))
            .collect(),
    );
    app.overlay_cost = ms(4_000);
    let entry = window_entry("w-1");
    let baseline = baseline(&app, &entry);

    let error = adapter_error(click_and_wait(&app, &entry, baseline.as_ref()));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(app.late_probes.load(Ordering::SeqCst), 0);
}
