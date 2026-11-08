use agent_desktop_core::{ErrorCode, ProcessId, Rect, WindowInfo, WindowState};

use serde_json::json;

use super::choose_page;
use crate::CdpConnection;
use crate::channel::browser_link::BrowserLink;
use crate::channel::test_support::{attach, deadline, process, targets, visibility, window_of};
use crate::transport::scripted_transport::ScriptedTransport;

fn link(transport: ScriptedTransport) -> BrowserLink {
    BrowserLink::new(process(), CdpConnection::new(transport.boxed()))
}

fn window_at(left: f64) -> WindowInfo {
    titled_window_at("Docs", [left, 0.0, 800.0, 600.0])
}

fn titled_window_at(title: &str, [x, y, width, height]: [f64; 4]) -> WindowInfo {
    WindowInfo {
        id: "w".into(),
        title: title.into(),
        app: "Browser".into(),
        pid: ProcessId::new(41),
        process_instance: Some("inst-41".into()),
        bounds: Some(Rect {
            x,
            y,
            width,
            height,
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

fn window_sized(transport: ScriptedTransport, id: i64, left: f64) -> ScriptedTransport {
    transport.expect(
        "Browser.getWindowForTarget",
        json!({ "windowId": id, "bounds": { "left": left, "top": 0, "width": 800, "height": 600 } }),
    )
}

fn one_window(pages: &[(&str, &str, &str)]) -> ScriptedTransport {
    let mut transport = targets(ScriptedTransport::new(), pages);
    for _ in pages {
        transport = window_sized(transport, 1, 0.0);
    }
    transport
}

#[test]
fn a_covered_window_shows_the_tab_its_title_names_without_reading_visibility() {
    let transport = one_window(&[
        ("A", "https://a/", "Mail"),
        ("B", "https://b/", "Docs"),
        ("C", "https://c/", "News"),
    ]);
    let mut link = link(transport);
    let window = titled_window_at("Docs - Browser", [0.0, 0.0, 800.0, 600.0]);
    let page = choose_page(&mut link, Some(&window), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn the_longest_tab_title_the_window_carries_wins() {
    let transport = one_window(&[
        ("A", "https://a/", "Docs"),
        ("B", "https://b/", "Docs - Guide"),
    ]);
    let mut link = link(transport);
    let window = titled_window_at("Docs - Guide - Browser", [0.0, 0.0, 800.0, 600.0]);
    let page = choose_page(&mut link, Some(&window), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn tabs_sharing_the_window_title_are_told_apart_by_visibility() {
    let mut transport = one_window(&[
        ("A", "https://a/", "Docs"),
        ("M", "https://m/", "Mail"),
        ("B", "https://b/", "Docs"),
    ]);
    transport = visibility(attach(transport, "A"), "hidden");
    transport = visibility(attach(transport, "B"), "visible");
    let mut link = link(transport);
    let page = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn same_frame_windows_are_told_apart_by_the_window_title() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "Docs"), ("B", "https://b/", "Mail")],
    );
    transport = window_sized(window_sized(transport, 1, 0.0), 2, 0.0);
    let mut link = link(transport);
    let window = titled_window_at("Mail", [0.0, 0.0, 800.0, 600.0]);
    let page = choose_page(&mut link, Some(&window), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn same_frame_windows_nothing_separates_are_ambiguous_without_naming_window_id() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[
            ("A", "https://a/", "New Tab"),
            ("B", "https://b/", "New Tab"),
        ],
    );
    transport = window_sized(window_sized(transport, 1, 0.0), 2, 0.0);
    transport = visibility(attach(transport, "A"), "hidden");
    transport = visibility(attach(transport, "B"), "hidden");
    let mut link = link(transport);
    let window = titled_window_at("New Tab", [0.0, 0.0, 800.0, 600.0]);
    let error = choose_page(&mut link, Some(&window), deadline()).unwrap_err();
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "cdp_window_indistinct");
    assert_eq!(details["browser_window_ids"], json!([1, 2]));
    assert!(!error.suggestion.unwrap().contains("--window-id"));
}

fn scaled_choice(scale: f64) -> String {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "A"), ("B", "https://b/", "B")],
    );
    transport = window_sized(window_sized(transport, 1, 0.0), 2, 900.0);
    let mut link = link(transport);
    let device = [900.0 * scale, 0.0, 800.0 * scale, 600.0 * scale];
    let window = titled_window_at("Docs", device);
    choose_page(&mut link, Some(&window), deadline())
        .unwrap()
        .target_id
}

#[test]
fn a_window_in_device_pixels_finds_its_browser_window_at_any_scale() {
    assert_eq!(scaled_choice(1.0), "B");
    assert_eq!(scaled_choice(1.5), "B");
    assert_eq!(scaled_choice(2.0), "B");
}

#[test]
fn a_tab_whose_visibility_read_fails_is_skipped() {
    let mut transport = one_window(&[
        ("A", "https://a/", "A"),
        ("B", "https://b/", "B"),
        ("C", "https://c/", "C"),
    ]);
    transport = transport.expect_error("Target.attachToTarget", -32602, "No target with given id");
    transport = attach(transport, "B").expect_error("Page.getFrameTree", -32000, "Page crashed");
    transport = visibility(attach(transport, "C"), "visible");
    let mut link = link(transport);
    let page = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap();
    assert_eq!(page.target_id, "C");
}

#[test]
fn a_tab_that_never_answers_is_skipped() {
    let mut transport = one_window(&[("A", "https://a/", "A"), ("B", "https://b/", "B")]);
    transport = attach(transport, "A")
        .expect(
            "Page.getFrameTree",
            json!({ "frameTree": { "frame": { "id": "F" } } }),
        )
        .expect(
            "Page.createIsolatedWorld",
            json!({ "executionContextId": 5 }),
        )
        .expect_silence("Runtime.evaluate");
    transport = visibility(attach(transport, "B"), "visible");
    let mut link = link(transport);
    let page = choose_page(&mut link, Some(&window_at(0.0)), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}

#[test]
fn a_window_that_fits_only_by_size_is_confirmed_by_its_title() {
    let mut transport = targets(
        ScriptedTransport::new(),
        &[("A", "https://a/", "Docs"), ("B", "https://b/", "Mail")],
    );
    transport = window_sized(window_sized(transport, 1, 0.0), 2, 900.0);
    let mut link = link(transport);
    let window = titled_window_at("Mail", [3000.0, 0.0, 1200.0, 900.0]);
    let page = choose_page(&mut link, Some(&window), deadline()).unwrap();
    assert_eq!(page.target_id, "B");
}
