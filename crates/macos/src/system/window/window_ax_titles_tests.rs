use super::WindowAxTitles;
use crate::system::window::cg_window::WindowRecord;

fn record(title: Option<&str>, window_number: i64) -> WindowRecord {
    WindowRecord {
        app_name: "Fixture".into(),
        pid: 10,
        title: title.map(str::to_owned),
        window_number,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        presence: crate::system::window::window_presence::WindowPresence::Onscreen,
        process_instance: Some("instance-10".into()),
    }
}

#[test]
fn missing_cg_name_takes_the_bridged_ax_title() {
    let mut titles = WindowAxTitles::default();
    titles.record(7, Some("Quarterly".into()));
    let mut window = record(None, 7);

    titles.fill_missing(&mut window);

    assert_eq!(window.title.as_deref(), Some("Quarterly"));
}

#[test]
fn present_cg_name_is_never_replaced() {
    let mut titles = WindowAxTitles::default();
    titles.record(7, Some("Accessibility title".into()));
    let mut window = record(Some("CoreGraphics title"), 7);

    titles.fill_missing(&mut window);

    assert_eq!(window.title.as_deref(), Some("CoreGraphics title"));
}

#[test]
fn conflicting_bridged_titles_leave_the_title_unknown() {
    let mut titles = WindowAxTitles::default();
    titles.record(7, Some("First".into()));
    titles.record(7, Some("Second".into()));
    let mut window = record(None, 7);

    titles.fill_missing(&mut window);

    assert_eq!(window.title, None);
    assert!(titles.contains(7));
}

#[test]
fn an_untitled_second_claim_also_makes_the_title_unknown() {
    let mut titles = WindowAxTitles::default();
    titles.record(7, Some("First".into()));
    titles.record(7, None);
    titles.record(7, Some("First".into()));

    assert_eq!(titles.title_for(7), None);
}

#[test]
fn agreeing_bridged_titles_stay_proven() {
    let mut titles = WindowAxTitles::default();
    titles.record(7, Some("Same".into()));
    titles.record(7, Some("Same".into()));

    assert_eq!(titles.title_for(7), Some("Same"));
}

#[test]
fn a_window_with_no_bridged_ax_window_gets_no_title() {
    let mut titles = WindowAxTitles::default();
    titles.record(8, Some("Other window".into()));
    let mut window = record(None, 7);

    titles.fill_missing(&mut window);

    assert_eq!(window.title, None);
    assert!(!titles.contains(7));
}

#[test]
fn an_empty_ax_title_is_no_title() {
    let mut titles = WindowAxTitles::default();
    titles.record(7, Some(String::new()));

    assert_eq!(titles.title_for(7), None);
    assert!(titles.contains(7));
}
