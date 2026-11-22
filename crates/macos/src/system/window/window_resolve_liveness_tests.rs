use super::*;
use crate::system::window::window_ax_state::WindowAxState;
use crate::system::window::window_presence::WindowPresence;
use std::cell::Cell;
use std::time::{Duration, Instant};

const NUMBER: i64 = 42;

struct FakeSources {
    record: WindowRecord,
    published: bool,
    complete: bool,
    on_any_space: Option<bool>,
    element: bool,
    ax_reads: Cell<usize>,
}

impl FakeSources {
    fn new(presence: WindowPresence) -> Self {
        Self {
            record: WindowRecord {
                app_name: "TextEdit".into(),
                pid: current_pid(),
                title: Some("Fixture".into()),
                window_number: NUMBER,
                bounds: agent_desktop_core::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                presence,
                process_instance: Some(instance()),
            },
            published: false,
            complete: true,
            on_any_space: Some(false),
            element: false,
            ax_reads: Cell::new(0),
        }
    }
}

impl WindowSources for FakeSources {
    fn record(&self, number: i64, _: Instant) -> Result<Option<WindowRecord>, AdapterError> {
        Ok((number == NUMBER).then(|| self.record.clone()))
    }

    fn ax_state(&self, _: i32, _: Instant) -> Result<WindowAxState, AdapterError> {
        self.ax_reads.set(self.ax_reads.get() + 1);
        Ok(WindowAxState {
            focused: None,
            minimized_by_id: rustc_hash::FxHashMap::default(),
            accessible_window_ids: if self.published {
                rustc_hash::FxHashSet::from_iter([NUMBER])
            } else {
                rustc_hash::FxHashSet::default()
            },
            window_numbers_complete: self.complete,
            titles: Default::default(),
        })
    }

    fn is_on_any_space(&self, _: i64) -> Option<bool> {
        self.on_any_space
    }

    fn element(
        &self,
        pid: i32,
        _: i64,
        _: Option<&str>,
        _: Instant,
    ) -> Result<Option<AXElement>, AdapterError> {
        Ok(self.element.then(|| crate::tree::element_for_pid(pid)))
    }
}

fn current_pid() -> i32 {
    i32::try_from(std::process::id()).unwrap()
}

fn instance() -> String {
    crate::system::process::process_identity::token_for_pid(current_pid())
        .unwrap()
        .expect("current test process has an identity token")
}

fn requested() -> WindowInfo {
    WindowInfo {
        id: format!("w-{NUMBER}"),
        title: "Fixture".into(),
        app: "TextEdit".into(),
        pid: agent_desktop_core::ProcessId::try_from(current_pid()).unwrap(),
        process_instance: Some(instance()),
        bounds: None,
        state: agent_desktop_core::WindowState::default(),
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn element_error(sources: &FakeSources) -> AdapterError {
    match resolve_window_element_strict(&requested(), deadline(), sources) {
        Ok(_) => panic!("the element path resolved a window it should refuse"),
        Err(error) => error,
    }
}

fn is_not_accessible(error: &AdapterError) -> bool {
    error.code == ErrorCode::ActionNotSupported
        && error.details.as_ref().map(|details| &details["kind"])
            == Some(&serde_json::json!("window_without_accessibility_element"))
}

#[test]
fn strict_resolve_reports_an_ordered_out_window_closed() {
    let sources = FakeSources::new(WindowPresence::Offscreen);
    let error = resolve_window_strict_with(&requested(), deadline(), &sources).unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn strict_resolve_keeps_a_window_on_another_space() {
    let mut sources = FakeSources::new(WindowPresence::Offscreen);
    sources.on_any_space = Some(true);
    let resolved = resolve_window_strict_with(&requested(), deadline(), &sources).unwrap();
    assert_eq!(resolved.id, format!("w-{NUMBER}"));
    assert_eq!(resolved.state.visible, Some(false));
}

#[test]
fn strict_resolve_keeps_an_offscreen_window_without_proof() {
    let mut unknown_space = FakeSources::new(WindowPresence::Offscreen);
    unknown_space.on_any_space = None;
    assert!(resolve_window_strict_with(&requested(), deadline(), &unknown_space).is_ok());
    let mut incomplete = FakeSources::new(WindowPresence::Offscreen);
    incomplete.complete = false;
    assert!(resolve_window_strict_with(&requested(), deadline(), &incomplete).is_ok());
}

#[test]
fn strict_resolve_keeps_a_hidden_application_window_it_still_publishes() {
    let mut sources = FakeSources::new(WindowPresence::Offscreen);
    sources.published = true;
    assert!(resolve_window_strict_with(&requested(), deadline(), &sources).is_ok());
}

#[test]
fn strict_resolve_reports_a_window_gone_from_core_graphics_closed() {
    let sources = FakeSources::new(WindowPresence::Offscreen);
    let mut gone = requested();
    gone.id = "w-43".into();
    let error = resolve_window_strict_with(&gone, deadline(), &sources).unwrap_err();
    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn element_path_keeps_not_accessible_for_a_window_on_another_space() {
    let mut sources = FakeSources::new(WindowPresence::Offscreen);
    sources.on_any_space = Some(true);
    let error = element_error(&sources);
    assert!(is_not_accessible(&error), "{error:?}");
}

#[test]
fn element_path_keeps_not_accessible_without_proof() {
    let mut sources = FakeSources::new(WindowPresence::Offscreen);
    sources.on_any_space = None;
    let error = element_error(&sources);
    assert!(is_not_accessible(&error), "{error:?}");
}

#[test]
fn element_path_reports_an_ordered_out_window_closed() {
    let sources = FakeSources::new(WindowPresence::Offscreen);
    let error = element_error(&sources);
    assert_eq!(error.code, ErrorCode::WindowNotFound);
}

#[test]
fn element_path_keeps_not_accessible_for_an_onscreen_window_without_reading_ax_state() {
    let sources = FakeSources::new(WindowPresence::Onscreen);
    let error = element_error(&sources);
    assert!(is_not_accessible(&error), "{error:?}");
    assert_eq!(sources.ax_reads.get(), 0);
}

#[test]
fn element_path_returns_a_published_element_without_reading_ax_state() {
    let mut sources = FakeSources::new(WindowPresence::Offscreen);
    sources.element = true;
    assert!(resolve_window_element_strict(&requested(), deadline(), &sources).is_ok());
    assert_eq!(sources.ax_reads.get(), 0);
}
