use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::Mutex;

/// A background application whose platform names `main` its main window and
/// `auxiliary` its floating panels, recording which windows it was asked about.
struct BackgroundApp {
    main: Option<&'static str>,
    auxiliary: &'static [&'static str],
    asked: Mutex<Vec<Vec<String>>>,
}

impl BackgroundApp {
    fn new(main: Option<&'static str>) -> Self {
        Self {
            main,
            auxiliary: &[],
            asked: Mutex::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<Vec<String>> {
        self.asked.lock().unwrap().clone()
    }
}

impl ObservationOps for BackgroundApp {
    fn is_auxiliary_window(&self, window: &WindowInfo, _deadline: crate::Deadline) -> bool {
        self.auxiliary.contains(&window.id.as_str())
    }

    fn main_window(&self, windows: &[WindowInfo], _deadline: crate::Deadline) -> Option<String> {
        let ids = windows.iter().map(|window| window.id.clone()).collect();
        self.asked.lock().unwrap().push(ids);
        self.main.map(str::to_owned)
    }
}
impl ActionOps for BackgroundApp {}
impl InputOps for BackgroundApp {}
impl SystemOps for BackgroundApp {}

fn background(id: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Untitled".into(),
        app: "Editor".into(),
        pid: crate::ProcessId::new(7),
        process_instance: Some("inst-7".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: false,
            accessible: true,
            minimized: Some(false),
            visible: Some(true),
            on_other_space: None,
        },
    }
}

fn choose(adapter: &BackgroundApp, windows: Vec<WindowInfo>) -> Result<WindowInfo, AppError> {
    select_app_window(
        adapter,
        windows,
        crate::AdapterError::new(ErrorCode::WindowNotFound, "none"),
        "More than one window matches the target",
        crate::Deadline::standard().unwrap(),
    )
}

#[test]
fn two_background_windows_resolve_to_the_one_the_platform_names_main() {
    let adapter = BackgroundApp::new(Some("w-2"));

    let chosen = choose(&adapter, vec![background("w-1"), background("w-2")]).unwrap();

    assert_eq!(chosen.id, "w-2");
    assert_eq!(adapter.asked(), vec![vec!["w-1", "w-2"]]);
}

#[test]
fn no_main_window_keeps_the_ambiguity_and_its_recovery() {
    let adapter = BackgroundApp::new(None);

    let Err(AppError::Adapter(error)) =
        choose(&adapter, vec![background("w-1"), background("w-2")])
    else {
        panic!("two windows with no main window are ambiguous");
    };

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Run 'list-windows' and retry with --window-id <id>.")
    );
    assert_eq!(error.details.unwrap()["candidate_count"], 2);
}

#[test]
fn a_main_window_outside_the_candidates_is_no_choice() {
    let adapter = BackgroundApp::new(Some("w-9"));

    let error = choose(&adapter, vec![background("w-1"), background("w-2")]).unwrap_err();

    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
}

#[test]
fn a_single_window_never_asks_for_the_main_window() {
    let adapter = BackgroundApp::new(Some("w-1"));

    let chosen = choose(&adapter, vec![background("w-1")]).unwrap();

    assert_eq!(chosen.id, "w-1");
    assert!(adapter.asked().is_empty());
}

#[test]
fn a_focused_window_wins_before_the_main_window_is_asked() {
    let adapter = BackgroundApp::new(Some("w-2"));
    let mut focused = background("w-1");
    focused.state.is_focused = true;

    let chosen = choose(&adapter, vec![focused, background("w-2")]).unwrap();

    assert_eq!(chosen.id, "w-1");
    assert!(adapter.asked().is_empty());
}

#[test]
fn the_main_window_is_chosen_among_the_visible_standard_windows() {
    let mut adapter = BackgroundApp::new(Some("w-3"));
    adapter.auxiliary = &["w-1"];
    let mut hidden = background("w-4");
    hidden.state.visible = Some(false);

    let chosen = choose(
        &adapter,
        vec![
            background("w-1"),
            background("w-2"),
            background("w-3"),
            hidden,
        ],
    )
    .unwrap();

    assert_eq!(chosen.id, "w-3");
    assert_eq!(adapter.asked(), vec![vec!["w-2", "w-3"]]);
}
