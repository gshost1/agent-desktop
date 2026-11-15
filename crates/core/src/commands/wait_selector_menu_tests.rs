use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::commands::wait_selector::{WaitSelectorInput, execute_after_action};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::wait::deadline::{VirtualClock, now};
use crate::wait::window_baseline::WindowBaseline;
use crate::{
    AccessibilityNode, AdapterError, AppError, ErrorCode, SnapshotSurface, WindowFilter,
    WindowInfo, context::CommandContext, refs::refs_test_support::HomeGuard,
    system::process_state::ProcessState,
};

const MAIN: &str = "w-main";
const OTHER: &str = "w-other";
const NEVER: Duration = Duration::from_secs(3600);

struct MenuApp {
    anchor: Instant,
    menu_opens: Duration,
    menu_is_application_surface: bool,
    reads: Mutex<Vec<(String, SnapshotSurface)>>,
}

impl MenuApp {
    fn new(menu_opens: Duration) -> Self {
        Self {
            anchor: now(),
            menu_opens,
            menu_is_application_surface: false,
            reads: Mutex::new(Vec::new()),
        }
    }

    fn reads_of(&self, surface: SnapshotSurface) -> Vec<String> {
        self.reads
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, read)| *read == surface)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn surface_tree(
        &self,
        id: &str,
        surface: SnapshotSurface,
    ) -> Result<AccessibilityNode, AdapterError> {
        self.reads.lock().unwrap().push((id.to_owned(), surface));
        match surface {
            SnapshotSurface::Menu
                if now().saturating_duration_since(self.anchor) >= self.menu_opens =>
            {
                Ok(node(
                    "menu",
                    "",
                    vec![node("menuitem", "Width:", Vec::new())],
                ))
            }
            SnapshotSurface::Menu => Err(AdapterError::surface_absent(
                SnapshotSurface::Menu,
                "The window does not present an open menu",
            )),
            SnapshotSurface::Window => Ok(node(
                "window",
                id,
                vec![node("button", "Cancel", Vec::new())],
            )),
            surface => Err(AdapterError::surface_absent(surface, "No such overlay")),
        }
    }
}

fn node(role: &str, name: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

fn window_info(id: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: id.into(),
        app: "TestApp".into(),
        pid: crate::ProcessId::new(1),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: id == MAIN,
            ..Default::default()
        },
    }
}

impl ObservationOps for MenuApp {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Menubar
            || (self.menu_is_application_surface && surface == SnapshotSurface::Menu)
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let ObservationRoot::Window(window) = root else {
            return Err(AdapterError::not_supported("observe_tree"));
        };
        crate::adapter::observed_tree(&root, self.surface_tree(&window.id, request.surface)?)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![window_info(MAIN), window_info(OTHER)])
    }

    fn get_tree(
        &self,
        window: &WindowInfo,
        opts: &crate::adapter::TreeOptions,
        _deadline: crate::Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        self.surface_tree(&window.id, opts.surface)
    }
}

impl ActionOps for MenuApp {}

impl InputOps for MenuApp {}

impl SystemOps for MenuApp {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window, SnapshotSurface::Menu]
    }

    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(window_info(&expected.id))
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<ProcessState, AdapterError> {
        Ok(ProcessState::Running)
    }
}

fn wait_after_action(app: &MenuApp, timeout_ms: u64) -> Result<Value, AppError> {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Options"));
    entry.source.source_app = Some("TestApp".into());
    entry.source.source_window_id = Some(MAIN.into());
    entry.source.source_window_title = Some(MAIN.into());
    let selector = crate::WaitSelector {
        query_raw: ":Width".into(),
        gone: false,
        timeout_ms,
    };
    let context = CommandContext::default().with_wait_selector(Some(selector.clone()));
    let baseline = WindowBaseline::before_action(
        app,
        &entry,
        &context,
        crate::Deadline::after(5_000).unwrap(),
    );
    execute_after_action(
        WaitSelectorInput {
            query_raw: selector.query_raw,
            gone: false,
            app: Some("TestApp".into()),
            window_id: Some(MAIN.into()),
            opts: crate::adapter::TreeOptions::default(),
            timeout_ms,
        },
        &entry,
        baseline.as_ref(),
        app,
        &CommandContext::default(),
    )
}

fn adapter_error(result: Result<Value, AppError>) -> AdapterError {
    match result {
        Err(AppError::Adapter(error)) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

#[test]
fn the_wait_reads_the_menu_the_action_opened() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = MenuApp::new(Duration::from_millis(300));

    let result = wait_after_action(&app, 10_000).unwrap();

    assert_eq!(result["matched_selector"], ":Width");
    assert_eq!(result["window"]["id"], MAIN);
    assert!(
        clock.elapsed() < Duration::from_secs(1),
        "{:?}",
        clock.elapsed()
    );
}

#[test]
fn the_menu_is_read_once_per_poll_through_the_focused_window() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let app = MenuApp::new(NEVER);

    let error = adapter_error(wait_after_action(&app, 2_000));

    assert_eq!(error.code, ErrorCode::Timeout);
    let menu_reads = app.reads_of(SnapshotSurface::Menu);
    assert!(!menu_reads.is_empty());
    assert!(
        menu_reads.iter().all(|id| id == MAIN),
        "the open menu is read through the focused window only: {menu_reads:?}"
    );
    let own_window_reads = app
        .reads_of(SnapshotSurface::Window)
        .into_iter()
        .filter(|id| id == MAIN)
        .count();
    assert!(
        menu_reads.len() <= own_window_reads,
        "{} menu reads over {own_window_reads} polls",
        menu_reads.len()
    );
}

#[test]
fn an_application_owned_menu_is_not_read_as_a_window_overlay() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut app = MenuApp::new(Duration::ZERO);
    app.menu_is_application_surface = true;

    let error = adapter_error(wait_after_action(&app, 2_000));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= Duration::from_secs(2));
    assert!(app.reads_of(SnapshotSurface::Menu).is_empty());
}
