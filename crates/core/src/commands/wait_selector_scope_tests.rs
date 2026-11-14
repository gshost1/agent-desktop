use std::time::{Duration, Instant};

use serde_json::Value;

use super::super::{WaitSelectorInput, execute_after_action};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::wait::deadline::{VirtualClock, now};
use crate::wait::window_baseline::WindowBaseline;
use crate::{
    AccessibilityNode, AdapterError, AppError, ErrorCode, WindowFilter, WindowInfo,
    context::CommandContext, refs::refs_test_support::HomeGuard,
    system::process_state::ProcessState,
};

const MAIN: &str = "w-main";
const DIALOG: &str = "w-dialog";
const OTHER: &str = "w-other";
const NEVER: Duration = Duration::from_secs(3600);

struct Window {
    id: &'static str,
    opens: Duration,
    closes: Duration,
}

struct ScriptedApp {
    anchor: Instant,
    windows: Vec<Window>,
    field: (&'static str, Duration),
    exits: Duration,
    unresponsive: (Duration, Duration),
}

impl ScriptedApp {
    fn new(windows: Vec<Window>, field: (&'static str, Duration)) -> Self {
        Self {
            anchor: now(),
            windows,
            field,
            exits: NEVER,
            unresponsive: (NEVER, NEVER),
        }
    }

    fn at(&self) -> Duration {
        now().saturating_duration_since(self.anchor)
    }

    fn is_open(&self, id: &str) -> bool {
        let at = self.at();
        at < self.exits
            && self
                .windows
                .iter()
                .any(|window| window.id == id && window.opens <= at && at < window.closes)
    }

    fn unresponsive_now(&self) -> bool {
        let at = self.at();
        self.unresponsive.0 <= at && at < self.unresponsive.1
    }

    fn tree(&self, id: &str) -> Result<AccessibilityNode, AdapterError> {
        if self.unresponsive_now() {
            return Err(AdapterError::app_unresponsive("TestApp")
                .with_details(serde_json::json!({ "retryable": true })));
        }
        if !self.is_open(id) {
            return Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                "window closed",
            ));
        }
        let field = if self.field.0 == id && self.field.1 <= self.at() {
            node("incrementor", "Width:", Vec::new())
        } else {
            node("button", "Cancel", Vec::new())
        };
        Ok(node("window", id, vec![field]))
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
        state: Default::default(),
    }
}

impl ObservationOps for ScriptedApp {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        let crate::locator::live_locator::ObservationRoot::Window(window) = root else {
            return Err(AdapterError::not_supported("observe_tree"));
        };
        crate::adapter::observed_tree(&root, self.tree(&window.id)?)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self
            .windows
            .iter()
            .filter(|window| self.is_open(window.id))
            .map(|window| window_info(window.id))
            .collect())
    }

    fn get_tree(
        &self,
        window: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
        _deadline: crate::Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        self.tree(&window.id)
    }
}

impl ActionOps for ScriptedApp {}

impl InputOps for ScriptedApp {}

impl SystemOps for ScriptedApp {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        if self.is_open(&expected.id) {
            return Ok(window_info(&expected.id));
        }
        Err(AdapterError::new(
            ErrorCode::WindowNotFound,
            "window closed",
        ))
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<ProcessState, AdapterError> {
        if self.at() >= self.exits {
            return Ok(ProcessState::Exited { code: None });
        }
        if self.unresponsive_now() {
            return Ok(ProcessState::Unresponsive);
        }
        Ok(ProcessState::Running)
    }
}

fn open(id: &'static str, opens_ms: u64) -> Window {
    Window {
        id,
        opens: Duration::from_millis(opens_ms),
        closes: NEVER,
    }
}

fn entry() -> crate::RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("menuitem", Some("Scale Image..."));
    entry.source.source_app = Some("TestApp".into());
    entry.source.source_window_id = Some(MAIN.into());
    entry.source.source_window_title = Some(MAIN.into());
    entry
}

fn wait_after_action(app: &ScriptedApp, timeout_ms: u64) -> Result<Value, AppError> {
    let entry = entry();
    let selector = crate::WaitSelector {
        query_raw: ":Width".into(),
        gone: false,
        timeout_ms,
    };
    let baseline = WindowBaseline::before_action(app, &entry, Some(&selector));
    execute_after_action(
        WaitSelectorInput {
            query_raw: selector.query_raw.clone(),
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
fn the_wait_follows_the_action_into_the_dialog_it_opened() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0), open(DIALOG, 300)], (DIALOG, ms(300)));

    let result = wait_after_action(&app, 10_000).unwrap();

    assert_eq!(result["window"]["id"], DIALOG);
    assert_eq!(result["matched_selector"], ":Width");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn a_window_open_before_the_action_never_satisfies_the_wait() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0), open(OTHER, 0)], (OTHER, ms(0)));

    let error = adapter_error(wait_after_action(&app, 2_000));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= ms(2_000));
}

#[test]
fn the_wait_follows_the_process_once_the_ref_window_closes() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let main = Window {
        id: MAIN,
        opens: Duration::ZERO,
        closes: ms(200),
    };
    let app = ScriptedApp::new(vec![main, open(OTHER, 0)], (OTHER, ms(500)));

    let result = wait_after_action(&app, 10_000).unwrap();

    assert_eq!(result["window"]["id"], OTHER);
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn an_exited_process_ends_the_wait_when_it_is_seen() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(vec![open(MAIN, 0)], (DIALOG, NEVER));
    app.exits = ms(400);

    let error = adapter_error(wait_after_action(&app, 10_000));

    assert_eq!(error.code, ErrorCode::AppNotFound);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "wait_target_gone");
    assert_eq!(details["process_state"], "exited");
    assert_eq!(details["selector"], ":Width");
    assert!(clock.elapsed() < ms(1_000), "{:?}", clock.elapsed());
}

#[test]
fn an_owner_unresponsive_for_the_limit_ends_the_wait() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(vec![open(MAIN, 0)], (DIALOG, NEVER));
    app.unresponsive = (ms(100), NEVER);

    let error = adapter_error(wait_after_action(&app, 10_000));

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "wait_owner_unresponsive");
    assert!(details["unresponsive_ms"].as_u64().unwrap() >= 5_000);
    assert!(clock.elapsed() >= ms(5_100), "{:?}", clock.elapsed());
    assert!(clock.elapsed() < ms(6_000), "{:?}", clock.elapsed());
}

#[test]
fn a_shorter_unresponsive_spell_still_reaches_a_slow_success() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(
        vec![open(MAIN, 0), open(DIALOG, 4_500)],
        (DIALOG, ms(4_500)),
    );
    app.unresponsive = (ms(100), ms(4_000));

    let result = wait_after_action(&app, 10_000).unwrap();

    assert_eq!(result["window"]["id"], DIALOG);
}

#[test]
fn the_default_budget_still_reaches_a_slow_success() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(
        vec![open(MAIN, 0), open(DIALOG, 8_000)],
        (DIALOG, ms(8_000)),
    );

    let result = wait_after_action(&app, crate::DEFAULT_POST_ACTION_WAIT_MS).unwrap();

    assert_eq!(result["window"]["id"], DIALOG);
    assert!(clock.elapsed() >= ms(8_000));
}

#[test]
fn the_default_budget_ends_an_effect_that_never_comes_at_ten_seconds() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0)], (DIALOG, NEVER));

    let error = adapter_error(wait_after_action(&app, crate::DEFAULT_POST_ACTION_WAIT_MS));

    assert_eq!(crate::DEFAULT_POST_ACTION_WAIT_MS, 10_000);
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.unwrap()["timeout_ms"], 10_000);
    assert!(clock.elapsed() >= ms(10_000), "{:?}", clock.elapsed());
    assert!(clock.elapsed() < ms(10_500), "{:?}", clock.elapsed());
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
