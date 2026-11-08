use agent_desktop_core::{
    CommandContext, PermissionReport,
    commands::session::{self, SessionAction},
};
use serde_json::json;

use super::execute;
use crate::{
    cli_args::batch::BatchArgs, dispatch::test_support::HomeGuard, test_noop_ops::NoopAdapter,
};

#[test]
fn a_repeated_session_end_runs_in_an_ended_session_as_it_does_standalone() {
    let _home = HomeGuard::new();
    let started = session::execute(SessionAction::Start {
        name: None,
        no_trace: true,
        screenshots: false,
        cursor_overlay: None,
    })
    .expect("session start");
    let session_id = started["session_id"].as_str().expect("session id");
    let end = json!({"command": "session", "session": session_id, "args": {"action": "end"}});

    let output = execute(
        BatchArgs {
            commands_json: json!([
                end,
                end,
                {"command": "clipboard-clear", "session": session_id, "args": {}}
            ])
            .to_string(),
            stop_on_error: false,
            timeout_ms: 60_000,
        },
        &NoopAdapter,
        &PermissionReport::default(),
        &CommandContext::default(),
    )
    .expect("batch runs");

    let results = &output["results"];
    assert_eq!(results[0]["ok"], true, "{output}");
    assert_eq!(results[1]["execution"], "completed", "{output}");
    assert_eq!(results[1]["ok"], true, "{output}");
    assert_eq!(results[1]["data"]["session_id"], session_id, "{output}");
    assert_eq!(
        results[2]["not_started_reason"], "session_ended",
        "{output}"
    );
    assert!(output.get("stopped").is_none(), "{output}");
}
