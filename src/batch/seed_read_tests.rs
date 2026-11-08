use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use agent_desktop_core::{
    AdapterError, AppInfo, CommandContext, Deadline, InteractionLease, OperationReport,
    OperationTarget, PermissionReport, ProcessId, SignalBaseline, SignalCompleteness, SignalFilter,
    WindowInfo, WindowState,
};
use serde_json::{Value, json};

use super::execute;
use crate::cli_args::batch::BatchArgs;

enum ReadMode {
    WindowOnSecondReadAfterClear,
    Fails,
    FailsAtDeadline,
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
    WindowInfo {
        id: "w-late".into(),
        title: "Late".into(),
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
            ReadMode::WindowOnSecondReadAfterClear if reads >= 2 => vec![late_window()],
            ReadMode::WindowOnSecondReadAfterClear => Vec::new(),
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
