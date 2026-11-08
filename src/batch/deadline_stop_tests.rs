use std::time::Duration;

use agent_desktop_core::{AdapterError, CommandContext, Deadline, PermissionReport};
use serde_json::{Value, json};

use crate::{batch::execute, cli_args::batch::BatchArgs, test_noop_ops::NoopAdapter};

fn run(
    adapter: &dyn agent_desktop_core::PlatformAdapter,
    commands: Value,
    stop_on_error: bool,
) -> Value {
    execute(
        BatchArgs {
            commands_json: commands.to_string(),
            stop_on_error,
            timeout_ms: 500,
        },
        adapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("entry timeout remains a structured batch result")
}

#[test]
fn a_batch_deadline_timeout_stops_as_deadline_with_or_without_stop_on_error() {
    for stop_on_error in [false, true] {
        let output = run(
            &NoopAdapter,
            json!([
                {"command": "wait", "args": {"ms": 5_000}},
                {"command": "version", "args": {}}
            ]),
            stop_on_error,
        );

        assert_eq!(output["results"][0]["error"]["code"], "TIMEOUT", "{output}");
        assert_eq!(output["stopped"]["reason"], "deadline", "{output}");
        assert_eq!(output["stopped"]["after_index"], 0, "{output}");
    }
}

#[test]
fn a_batch_deadline_timeout_on_the_last_entry_stops_as_deadline() {
    for stop_on_error in [false, true] {
        let output = run(
            &NoopAdapter,
            json!([{"command": "wait", "args": {"ms": 5_000}}]),
            stop_on_error,
        );

        assert_eq!(output["results"][0]["error"]["code"], "TIMEOUT", "{output}");
        assert_eq!(
            output["stopped"],
            json!({"reason": "deadline", "after_index": 0}),
            "{output}"
        );
    }
}

struct StalledStepAdapter;

impl agent_desktop_core::ObservationOps for StalledStepAdapter {}
impl agent_desktop_core::ActionOps for StalledStepAdapter {}
impl agent_desktop_core::InputOps for StalledStepAdapter {}

impl agent_desktop_core::SystemOps for StalledStepAdapter {
    fn list_displays(
        &self,
        _deadline: Deadline,
    ) -> Result<Vec<agent_desktop_core::DisplayInfo>, AdapterError> {
        let step = Deadline::after(5_000)?;
        while !step.is_expired() {
            std::thread::sleep(Duration::from_millis(2));
        }
        Err(AdapterError::timeout("display enumeration stalled")
            .with_details(json!({ "timeout_ms": 50 })))
    }
}

#[test]
fn a_sub_step_timeout_capped_by_the_batch_deadline_stops_as_deadline() {
    for stop_on_error in [false, true] {
        let output = run(
            &StalledStepAdapter,
            json!([
                {"command": "list-displays", "args": {}},
                {"command": "version", "args": {}}
            ]),
            stop_on_error,
        );

        assert_eq!(output["results"][0]["error"]["code"], "TIMEOUT", "{output}");
        assert_eq!(
            output["stopped"],
            json!({"reason": "deadline", "after_index": 0}),
            "{output}"
        );
    }
}
