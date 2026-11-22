use super::*;

#[test]
fn a_background_menu_bar_click_reads_the_window_its_command_acts_in() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, false, Field::InWindow(ms(300))),
        pane("w-2", OURS, false, Field::Nowhere),
    ]);
    let entry = menubar_entry();
    let baseline = baseline(&app, &entry);
    assert!(baseline.is_some());

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-1");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_background_menu_bar_click_ignores_a_window_behind_the_one_it_acts_in() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, false, Field::Nowhere),
        pane("w-2", OURS, false, Field::InWindow(Duration::ZERO)),
    ]);
    let entry = menubar_entry();
    let baseline = baseline(&app, &entry);

    let error = adapter_error(click_and_wait(&app, &entry, baseline.as_ref()));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= ms(10_000), "{:?}", clock.elapsed());
}

#[test]
fn a_menu_bar_click_without_a_baseline_still_reads_the_focused_window() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-2", OURS, false, Field::InWindow(Duration::ZERO)),
        pane("w-1", OURS, true, Field::InWindow(ms(300))),
    ]);

    let result = click_and_wait(&app, &menubar_entry(), None).unwrap();

    assert_eq!(result["window"]["id"], "w-1");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_background_menu_bar_click_without_a_baseline_reads_only_the_front_window() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, false, Field::InWindow(ms(300))),
        pane("w-2", OURS, false, Field::InWindow(Duration::ZERO)),
    ]);

    let result = click_and_wait(&app, &menubar_entry(), None).unwrap();

    assert_eq!(result["window"]["id"], "w-1");
    assert!(clock.elapsed() >= ms(300), "{:?}", clock.elapsed());
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

/// A background app lists a floating inspector above its document, neither
/// focused. The menu command acts in the document, so the wait reads it
/// rather than the inspector the platform lists first.
#[test]
fn a_background_menu_bar_click_reads_the_document_behind_a_floating_panel() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("panel-inspector", OURS, false, Field::Nowhere),
        pane("w-document", OURS, false, Field::InWindow(ms(300))),
    ]);
    let entry = menubar_entry();
    let baseline = baseline(&app, &entry);
    assert!(baseline.is_some());

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-document");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_background_menu_bar_click_without_a_baseline_reads_the_document_behind_a_floating_panel() {
    let _home = HomeGuard::new();
    let app = TwoWindowApp::new(vec![
        pane("panel-inspector", OURS, false, Field::Nowhere),
        pane("w-document", OURS, false, Field::InWindow(Duration::ZERO)),
    ]);

    let result = click_and_wait(&app, &menubar_entry(), None).unwrap();

    assert_eq!(result["window"]["id"], "w-document");
}

#[test]
fn a_focused_floating_panel_stays_the_window_a_menu_bar_click_acts_in() {
    let _home = HomeGuard::new();
    let app = TwoWindowApp::new(vec![
        pane("w-document", OURS, false, Field::InWindow(Duration::ZERO)),
        pane(
            "panel-inspector",
            OURS,
            true,
            Field::InWindow(Duration::ZERO),
        ),
    ]);
    let entry = menubar_entry();
    let baseline = baseline(&app, &entry);

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "panel-inspector");
}
