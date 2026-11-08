use agent_desktop_core::{ErrorCode, ProcessId, Rect, WindowInfo, WindowState};

use super::choose_page;
use crate::CdpConnection;
use crate::channel::browser_link::BrowserLink;
use crate::channel::test_support::{attach, deadline, process, targets, visibility, window_of};
use crate::transport::scripted_transport::ScriptedTransport;

fn link(transport: ScriptedTransport) -> BrowserLink {
    BrowserLink::new(process(), CdpConnection::new(transport.boxed()), None)
}

fn window_at(left: f64) -> WindowInfo {
    WindowInfo {
        id: "w".into(),
        title: "Docs".into(),
        app: "Browser".into(),
        pid: ProcessId::new(41),
        process_instance: Some("inst-41".into()),
        bounds: Some(Rect {
            x: left,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        }),
        state: WindowState::default(),
    }
}

#[test]
fn a_lone_page_is_chosen_without_reading_visibility() {
    let transport = window_of(
        targets(ScriptedTransport::new(), &[("A", "https://a/", "A")]),
        1,
        0.0,
    );
    let mut link = link(transport);
    let page = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap();
    assert_eq!(page.target_id, "A");
}

#[test]
fn among_tabs_of_one_window_the_visible_tab_is_chosen() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "A"), ("B", "https://b/", "B")],
    );
    transport = window_of(window_of(transport, 1, 0.0), 1, 0.0);
    transport = visibility(attach(transport, "A"), "hidden");
    transport = visibility(attach(transport, "B"), "visible");
    let mut link = link(transport);
    let page = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn the_window_whose_bounds_match_holds_the_page() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "A"), ("B", "https://b/", "B")],
    );
    transport = window_of(window_of(transport, 1, 0.0), 2, 900.0);
    let mut link = link(transport);
    let page = choose_page(&mut link, Some(&window_at(901.0)), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn a_window_no_browser_window_matches_is_not_found() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "A"), ("B", "https://b/", "B")],
    );
    transport = window_of(window_of(transport, 1, 0.0), 2, 900.0);
    let mut link = link(transport);
    let error = choose_page(&mut link, Some(&window_at(400.0)), deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.details.unwrap()["kind"], "cdp_window_unmatched");
}

#[test]
fn a_window_with_no_visible_tab_is_not_found() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "A"), ("B", "https://b/", "B")],
    );
    transport = window_of(window_of(transport, 1, 0.0), 1, 0.0);
    transport = visibility(attach(transport, "A"), "hidden");
    transport = visibility(attach(transport, "B"), "hidden");
    let mut link = link(transport);
    let error = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn two_visible_tabs_without_a_window_to_tell_them_apart_are_ambiguous() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "A"), ("B", "https://b/", "B")],
    );
    transport = window_of(window_of(transport, 1, 0.0), 2, 900.0);
    transport = visibility(attach(transport, "A"), "visible");
    transport = visibility(attach(transport, "B"), "visible");
    let mut link = link(transport);
    let error = choose_page(&mut link, None, deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn a_browser_with_no_page_target_names_open_url() {
    let mut link = link(targets(ScriptedTransport::new(), &[]));
    let error = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(error.details.unwrap()["kind"], super::NO_PAGE_TARGET);
    let suggestion = error.suggestion.unwrap();
    assert!(suggestion.contains("open-url <url> --app <browser>"));
    assert!(!suggestion.contains("--new-tab"));
}
