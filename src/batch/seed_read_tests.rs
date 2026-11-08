use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use agent_desktop_core::{
    AdapterError, AppInfo, CommandContext, Deadline, InteractionLease, OperationReport,
    OperationTarget, PermissionReport, ProcessId, SignalBaseline, SignalCompleteness, SignalFilter,
    WindowInfo, WindowState,
};
use serde_json::{Value, json};

use crate::batch::execute;
use crate::cli_args::batch::BatchArgs;

enum ReadMode {
    WindowOnSecondReadAfterClear,
    SecondWindowOnThirdReadAfterClear,
    Fails,
    FailsAtDeadline,
    SucceedsAtDeadline,
}

struct ReadAdapter {
    mode: ReadMode,
    cleared: AtomicBool,
    clears: AtomicUsize,
    reads_after_clear: AtomicUsize,
}

fn adapter(mode: ReadMode) -> ReadAdapter {
    ReadAdapter {
        mode,
        cleared: AtomicBool::new(false),
        clears: AtomicUsize::new(0),
        reads_after_clear: AtomicUsize::new(0),
    }
}

fn late_window() -> WindowInfo {
    window("w-late", "Late")
}

fn window(id: &str, title: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: title.into(),
        app: "Fixture".into(),
        pid: ProcessId::new(42),
        process_instance: Some("fixture-42".into()),
        bounds: None,
        state: WindowState::default(),
    }
}

impl agent_desktop_core::ObservationOps for ReadAdapter {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "Fixture".into(),
            pid: ProcessId::new(42),
            bundle_id: Some("com.example.fixture".into()),
            process_instance: Some("fixture-42".into()),
            presentation: None,
        }])
    }
}

impl agent_desktop_core::ActionOps for ReadAdapter {}

impl agent_desktop_core::InputOps for ReadAdapter {
    fn clear_clipboard(&self, _lease: &InteractionLease) -> Result<OperationReport, AdapterError> {
        self.cleared.store(true, Ordering::SeqCst);
        self.clears.fetch_add(1, Ordering::SeqCst);
        Ok(OperationReport::delivered_unverified(
            "test",
            OperationTarget::Clipboard,
        ))
    }
}

impl agent_desktop_core::SystemOps for ReadAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }

    fn capture_signal_baseline(
        &self,
        _filter: &SignalFilter,
        deadline: Deadline,
    ) -> Result<SignalBaseline, AdapterError> {
        let reads = if self.cleared.load(Ordering::SeqCst) {
            self.reads_after_clear.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            0
        };
        let windows = match self.mode {
            ReadMode::Fails => return Err(AdapterError::internal("inventory unavailable")),
            ReadMode::FailsAtDeadline => {
                std::thread::sleep(deadline.remaining());
                return Err(deadline.timeout_error());
            }
            ReadMode::SucceedsAtDeadline => {
                std::thread::sleep(deadline.remaining());
                Vec::new()
            }
            ReadMode::WindowOnSecondReadAfterClear if reads >= 2 => vec![late_window()],
            ReadMode::SecondWindowOnThirdReadAfterClear if reads >= 3 => {
                vec![window("w-alpha", "Alpha"), window("w-beta", "Beta")]
            }
            ReadMode::SecondWindowOnThirdReadAfterClear if reads == 2 => {
                vec![window("w-alpha", "Alpha")]
            }
            ReadMode::WindowOnSecondReadAfterClear
            | ReadMode::SecondWindowOnThirdReadAfterClear => Vec::new(),
        };
        Ok(SignalBaseline {
            windows,
            apps: Vec::new(),
            surfaces: Vec::new(),
            completeness: SignalCompleteness::complete(),
        })
    }
}

fn run(adapter: &ReadAdapter, commands: Value, stop_on_error: bool, timeout_ms: u64) -> Value {
    execute(
        BatchArgs {
            commands_json: commands.to_string(),
            stop_on_error,
            timeout_ms,
        },
        adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .unwrap()
}

fn opened_wait(timeout: u64) -> Value {
    json!({
        "command": "wait",
        "args": {"event": "window-opened", "app": "Fixture", "timeout": timeout}
    })
}

#[test]
fn a_second_identical_wait_does_not_report_the_window_the_first_wait_reported() {
    let adapter = adapter(ReadMode::WindowOnSecondReadAfterClear);

    let value = run(
        &adapter,
        json!([
            {"command": "clipboard-clear", "args": {}},
            opened_wait(300),
            opened_wait(150)
        ]),
        false,
        60_000,
    );

    assert_eq!(
        value["results"][1]["data"]["event"]["window_id"], "w-late",
        "{value}"
    );
    assert_eq!(value["results"][2]["ok"], false, "{value}");
    assert_eq!(value["results"][2]["error"]["code"], "TIMEOUT", "{value}");
}

#[test]
fn a_baseline_read_that_runs_out_the_batch_deadline_stops_as_deadline() {
    let adapter = adapter(ReadMode::FailsAtDeadline);

    let value = run(
        &adapter,
        json!([{"command": "clipboard-clear", "args": {}}, opened_wait(100)]),
        false,
        200,
    );

    assert_eq!(adapter.clears.load(Ordering::SeqCst), 0, "{value}");
    assert_eq!(
        value["results"][0]["not_started_reason"], "deadline",
        "{value}"
    );
    assert_eq!(value["stopped"], json!({"reason": "deadline", "index": 0}));
}

#[test]
fn an_early_read_failure_lets_the_action_run_and_the_wait_reports_it() {
    for stop_on_error in [false, true] {
        let adapter = adapter(ReadMode::Fails);

        let value = run(
            &adapter,
            json!([
                {"command": "clipboard-clear", "args": {}},
                {"command": "version", "args": {}},
                opened_wait(100),
                {"command": "version", "args": {}}
            ]),
            stop_on_error,
            60_000,
        );

        let results = &value["results"];
        assert_eq!(adapter.clears.load(Ordering::SeqCst), 1, "{value}");
        assert_eq!(results[0]["ok"], true, "{value}");
        assert_eq!(results[1]["ok"], true, "{value}");
        assert_eq!(results[2]["execution"], "not_started", "{value}");
        assert_eq!(
            results[2]["not_started_reason"], "pre_action_baseline_failed",
            "{value}"
        );
        assert_eq!(
            results[2]["error"]["details"]["read_at_index"], 0,
            "{value}"
        );
        if stop_on_error {
            assert_eq!(
                value["stopped"],
                json!({"reason": "stop_on_error", "index": 2})
            );
        } else {
            assert_eq!(results[3]["ok"], true, "{value}");
            assert!(value.get("stopped").is_none(), "{value}");
        }
    }
}

fn titled_wait(title: &str) -> Value {
    json!({
        "command": "wait",
        "args": {"event": "window-opened", "app": "Fixture", "window": title, "timeout": 300}
    })
}

#[test]
fn a_following_wait_for_another_window_still_sees_a_window_opened_during_the_first_wait() {
    let adapter = adapter(ReadMode::SecondWindowOnThirdReadAfterClear);

    let value = run(
        &adapter,
        json!([
            {"command": "clipboard-clear", "args": {}},
            titled_wait("Alpha"),
            titled_wait("Beta")
        ]),
        false,
        60_000,
    );

    assert_eq!(
        value["results"][1]["data"]["event"]["window_id"], "w-alpha",
        "{value}"
    );
    assert_eq!(
        value["results"][2]["data"]["event"]["window_id"], "w-beta",
        "{value}"
    );
}

#[test]
fn an_adjacent_read_failure_before_a_read_only_entry_is_held_for_the_wait() {
    let adapter = adapter(ReadMode::Fails);

    let value = run(
        &adapter,
        json!([{"command": "version", "args": {}}, opened_wait(100)]),
        false,
        60_000,
    );

    assert_eq!(value["results"][0]["ok"], true, "{value}");
    assert_eq!(
        value["results"][1]["not_started_reason"], "pre_action_baseline_failed",
        "{value}"
    );
    assert!(value.get("stopped").is_none(), "{value}");
}

#[test]
fn a_read_that_uses_up_the_batch_deadline_does_not_start_the_entry() {
    let adapter = adapter(ReadMode::SucceedsAtDeadline);

    let value = run(
        &adapter,
        json!([{"command": "clipboard-clear", "args": {}}, opened_wait(100)]),
        false,
        200,
    );

    assert_eq!(adapter.clears.load(Ordering::SeqCst), 0, "{value}");
    assert_eq!(
        value["results"][0]["not_started_reason"], "deadline",
        "{value}"
    );
    assert_eq!(value["stopped"], json!({"reason": "deadline", "index": 0}));
}

#[test]
fn a_wait_not_started_for_its_failed_read_still_takes_the_read_it_owes_the_next_wait() {
    let focus_wait = json!({"command": "wait", "args": {"event": "focus-changed", "timeout": 100}});
    for next_wait in [opened_wait(100), focus_wait] {
        let adapter = adapter(ReadMode::Fails);

        let value = run(
            &adapter,
            json!([
                {"command": "clipboard-clear", "args": {}},
                {"command": "version", "args": {}},
                opened_wait(100),
                next_wait
            ]),
            false,
            60_000,
        );

        let results = &value["results"];
        assert_eq!(
            results[2]["not_started_reason"], "pre_action_baseline_failed",
            "{value}"
        );
        assert_eq!(
            results[3]["not_started_reason"], "pre_action_baseline_failed",
            "{value}"
        );
        assert_eq!(
            results[3]["error"]["details"]["read_at_index"], 2,
            "{value}"
        );
    }
}
