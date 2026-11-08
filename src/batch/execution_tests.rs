use crate::batch::execute;
use crate::batch::result_entry::bounded_entry;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_desktop_core::{CommandContext, InteractionLease, PermissionReport, PermissionState};
use serde_json::json;

use super::*;

struct CountingAdapter {
    clears: AtomicUsize,
}

impl agent_desktop_core::ObservationOps for CountingAdapter {}
impl agent_desktop_core::ActionOps for CountingAdapter {
    fn execute_action(
        &self,
        _handle: &agent_desktop_core::NativeHandle,
        request: agent_desktop_core::ActionRequest,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::ActionResult, agent_desktop_core::AdapterError> {
        self.clears.fetch_add(1, Ordering::SeqCst);
        Ok(agent_desktop_core::ActionResult::from_execution(
            &request.action,
            Vec::new(),
        ))
    }
}

impl agent_desktop_core::InputOps for CountingAdapter {
    fn clear_clipboard(
        &self,
        _lease: &InteractionLease,
    ) -> Result<agent_desktop_core::OperationReport, AdapterError> {
        self.clears.fetch_add(1, Ordering::SeqCst);
        Ok(agent_desktop_core::OperationReport::delivered_unverified(
            "test",
            agent_desktop_core::OperationTarget::Clipboard,
        ))
    }
}

impl agent_desktop_core::SystemOps for CountingAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::InteractionLease, AdapterError> {
        agent_desktop_core::InteractionLease::guarded(deadline, ())
    }
}

fn args(commands: Value, timeout_ms: u64) -> BatchArgs {
    BatchArgs {
        commands_json: commands.to_string(),
        stop_on_error: false,
        timeout_ms,
    }
}

fn adapter() -> CountingAdapter {
    CountingAdapter {
        clears: AtomicUsize::new(0),
    }
}

#[test]
fn validates_every_entry_before_the_first_side_effect() {
    let adapter = adapter();
    for commands in [
        json!([
            {"command": "clipboard-clear", "args": {}},
            {"command": "missing", "args": {}}
        ]),
        json!([
            {"command": "clipboard-clear", "args": {}},
            {"command": "click", "args": {"ref_id": "not-a-ref"}}
        ]),
    ] {
        let error = execute(
            args(commands, 60_000),
            &adapter,
            &PermissionReport::default(),
            &CommandContext::default(),
        )
        .expect_err("a malformed or policy-invalid later entry rejects the whole batch");
        assert_eq!(error.code(), "INVALID_ARGS");
    }
    let denied = PermissionReport {
        accessibility: PermissionState::Denied {
            suggestion: "grant accessibility".into(),
        },
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };
    let error = execute(
        args(
            json!([
                {"command": "clipboard-clear", "args": {}},
                {"command": "snapshot", "args": {}}
            ]),
            60_000,
        ),
        &adapter,
        &denied,
        &CommandContext::default(),
    )
    .expect_err("a later permission failure rejects the whole batch");
    assert_eq!(error.code(), "PERM_DENIED");
    assert_eq!(adapter.clears.load(Ordering::SeqCst), 0);
}

#[test]
fn dispatches_each_entry_at_most_once() {
    let adapter = adapter();
    let output = execute(
        args(json!([{"command": "clipboard-clear", "args": {}}]), 60_000),
        &adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("valid batch executes");

    assert_eq!(adapter.clears.load(Ordering::SeqCst), 1);
    assert_eq!(output["semantics"]["batch_retries"], false);
    assert_eq!(output["semantics"]["atomic"], false);
    assert_eq!(output["results"][0]["execution"], "completed");
    assert_eq!(output["completed_entries"], 1);
    assert_eq!(output["not_started_entries"], 0);
}

#[test]
fn expired_batch_never_starts_the_entry() {
    let adapter = adapter();
    let output = execute(
        args(json!([{"command": "clipboard-clear", "args": {}}]), 0),
        &adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("deadline is reported in the batch result");

    assert_eq!(adapter.clears.load(Ordering::SeqCst), 0);
    assert_eq!(output["results"][0]["execution"], "not_started");
    assert_eq!(output["completed_entries"], 0);
    assert_eq!(output["not_started_entries"], output["total_entries"]);
    assert_eq!(output["results"][0]["not_started_reason"], "deadline");
    assert_eq!(
        output["results"][0]["error"]["disposition"]["delivery"],
        "not_delivered"
    );
    assert_eq!(
        output["results"][0]["error"]["disposition"]["retry"],
        "safe"
    );
    assert_eq!(output["stopped"]["reason"], "deadline");
}

#[test]
fn baseline_failure_prevents_the_producer_action() {
    let adapter = adapter();
    let output = execute(
        args(
            json!([
                {"command": "press", "args": {"combo": "a"}},
                {"command": "wait", "args": {"event": "window-opened", "timeout": 100}}
            ]),
            60_000,
        ),
        &adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("runtime baseline failure remains a structured batch result");

    assert_eq!(adapter.clears.load(Ordering::SeqCst), 0);
    assert_eq!(output["results"][0]["execution"], "not_started");
    assert_eq!(output["completed_entries"], 0);
    assert_eq!(output["not_started_entries"], output["total_entries"]);
    assert_eq!(
        output["results"][0]["not_started_reason"],
        "pre_action_baseline_failed"
    );
    assert_eq!(output["stopped"]["blocked_index"], 0);
    assert_eq!(output["stopped"]["wait_index"], 1);
    assert_eq!(output["stopped"]["reason"], "pre_action_baseline_failed");
}

#[test]
fn entry_count_and_output_are_bounded() {
    let adapter = adapter();
    let input_error = execute(
        BatchArgs {
            commands_json: " ".repeat(MAX_BATCH_JSON_BYTES + 1),
            stop_on_error: false,
            timeout_ms: 60_000,
        },
        &adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect_err("oversized JSON is rejected before parsing");
    assert_eq!(input_error.code(), "INVALID_ARGS");

    let commands = (0..=MAX_BATCH_ENTRIES)
        .map(|_| json!({"command": "version", "args": {}}))
        .collect::<Vec<_>>();
    let error = execute(
        args(Value::Array(commands), 60_000),
        &adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect_err("oversized entry count is rejected");
    assert_eq!(error.code(), "INVALID_ARGS");

    let huge = Ok(json!({
        "payload": "x".repeat(MAX_BATCH_OUTPUT_BYTES),
        "disposition": {
            "delivery": "delivered_verified",
            "retry": "unsafe"
        }
    }));
    let (entry, oversized) = bounded_entry(0, "click", CommandEffect::AppInput, huge, 0);
    assert!(oversized);
    assert!(serialized_size(&entry) < super::super::result_entry::OUTPUT_METADATA_RESERVE);
    assert_eq!(
        entry["error"]["disposition"]["delivery"],
        "delivered_verified"
    );
    assert_eq!(entry["error"]["disposition"]["retry"], "unsafe");
}

#[test]
fn an_oversized_entry_that_sends_no_input_is_not_delivered() {
    for (command, effect) in [
        ("snapshot", CommandEffect::ReadOnly),
        ("screenshot", CommandEffect::LocalWrite),
    ] {
        let huge = Ok(json!({ "payload": "x".repeat(MAX_BATCH_OUTPUT_BYTES) }));

        let (entry, oversized) = bounded_entry(0, command, effect, huge, 0);

        assert!(oversized);
        assert_eq!(entry["error"]["disposition"]["delivery"], "not_delivered");
        assert_eq!(entry["error"]["disposition"]["retry"], "safe");
    }
}

#[test]
fn an_oversized_successful_action_without_a_report_is_delivered_unverified() {
    let huge = Ok(json!({ "payload": "x".repeat(MAX_BATCH_OUTPUT_BYTES) }));

    let (entry, oversized) = bounded_entry(0, "launch", CommandEffect::AppInput, huge, 0);

    assert!(oversized);
    assert_eq!(
        entry["error"]["disposition"]["delivery"],
        "delivered_unverified"
    );
    assert_eq!(entry["error"]["disposition"]["retry"], "unsafe");
}

#[test]
fn a_trace_failure_body_over_the_output_limit_is_omitted() {
    let body = json!({
        "results": [{ "data": { "payload": "x".repeat(MAX_BATCH_OUTPUT_BYTES) } }],
        "total_entries": 1,
        "completed_entries": 1,
    });

    let details = crate::batch::trace_failure_details(body);

    assert_eq!(details["result_omitted"], true);
    assert_eq!(details["total_entries"], 1);
    assert!(details.get("batch").is_none());
    assert!(serialized_size(&details) < 1024);
}

struct RecordingReadAdapter {
    received: std::sync::Mutex<Vec<Duration>>,
}

impl agent_desktop_core::ObservationOps for RecordingReadAdapter {}
impl agent_desktop_core::ActionOps for RecordingReadAdapter {}
impl agent_desktop_core::InputOps for RecordingReadAdapter {}

impl agent_desktop_core::SystemOps for RecordingReadAdapter {
    fn list_displays(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<Vec<agent_desktop_core::DisplayInfo>, AdapterError> {
        self.received
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(deadline.remaining());
        Err(deadline.timeout_error())
    }
}

#[test]
fn inherited_batch_deadline_bounds_slow_non_action_command() {
    let adapter = RecordingReadAdapter {
        received: std::sync::Mutex::new(Vec::new()),
    };
    let output = execute(
        args(json!([{"command": "list-displays", "args": {}}]), 25),
        &adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("entry timeout remains a structured batch result");

    let received = adapter
        .received
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    assert_eq!(received.len(), 1, "the entry reached the adapter once");
    assert!(
        received[0] <= Duration::from_millis(25),
        "the adapter was handed {:?}, not the 25 ms batch budget",
        received[0]
    );
    assert_eq!(output["results"][0]["error"]["code"], "TIMEOUT");
}

#[test]
fn inherited_batch_deadline_caps_sleep_without_variant_rewriting() {
    let output = execute(
        args(json!([{"command": "wait", "args": {"ms": 5_000}}]), 25),
        &adapter(),
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("entry timeout remains a structured batch result");

    assert_eq!(output["results"][0]["error"]["code"], "TIMEOUT");
}
