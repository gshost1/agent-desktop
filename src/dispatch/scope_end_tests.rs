use std::io::Write;

use agent_desktop_core::session::{SessionTraceMode, StartSessionOptions};
use agent_desktop_core::{AppError, PermissionReport, context::CommandContext};
use clap::Parser;
use serde_json::{Value, json};

use crate::cli::Cli;
use crate::dispatch::test_support::HomeGuard;
use crate::test_noop_ops::NoopAdapter;

fn exhaust_trace_budget(session_id: &str, events_used: usize) {
    let ledger = agent_desktop_core::session::trace_dir(session_id)
        .expect("trace directory")
        .join(".trace-usage.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(ledger)
        .expect("the ledger exists after the first event");
    file.write_all(json!([0, events_used, 1]).to_string().as_bytes())
        .expect("ledger rewritten");
}

fn batch(entries: Value) -> crate::cli::Commands {
    Cli::try_parse_from(["agent-desktop", "batch", &entries.to_string()])
        .expect("batch parses")
        .command
        .expect("subcommand is present")
}

fn strict_session(events_used: usize) -> CommandContext {
    let manifest = agent_desktop_core::session::start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        ..Default::default()
    })
    .expect("session starts");
    let context = CommandContext::new(Some(manifest.id.clone()), None, true).expect("context");
    context
        .trace("test.ready", json!({}))
        .expect("the first event opens the trace");
    exhaust_trace_budget(&manifest.id, events_used);
    context
}

fn run(command: crate::cli::Commands, context: &CommandContext) -> Result<Value, AppError> {
    crate::dispatch::dispatch(command, &NoopAdapter, &PermissionReport::default(), context)
}

fn two_versions() -> crate::cli::Commands {
    batch(json!([
        {"command": "version", "args": {}},
        {"command": "version", "args": {}}
    ]))
}

#[test]
fn a_strict_trace_failure_at_the_batch_end_keeps_every_entry_result() {
    let _home = HomeGuard::new();
    let context = strict_session(99_999);

    let result = run(two_versions(), &context);

    let Err(AppError::Adapter(error)) = result else {
        panic!("a strict trace failure still fails the batch, got {result:?}");
    };
    let details = error
        .details
        .expect("the batch body travels with the error");
    assert_eq!(details["kind"], "batch_trace_end_failed");
    assert_eq!(details["batch"]["total_entries"], 2);
    let results = details["batch"]["results"]
        .as_array()
        .expect("per-entry results are kept");
    assert_eq!(results.len(), 2);
    for entry in results {
        assert_eq!(entry["error"]["disposition"]["delivery"], "not_delivered");
    }
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

#[test]
fn a_read_only_batch_whose_trace_end_fails_sent_nothing() {
    let _home = HomeGuard::new();
    let context = strict_session(99_995);

    let result = run(two_versions(), &context);

    let Err(AppError::Adapter(error)) = result else {
        panic!("the batch end must fail, got {result:?}");
    };
    let details = error.details.expect("details");
    let results = details["batch"]["results"].as_array().expect("results");
    assert!(
        results.iter().all(|entry| entry["ok"] == true),
        "{results:?}"
    );
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

#[test]
fn a_local_write_whose_trace_end_fails_after_success_is_delivered_unverified() {
    let _home = HomeGuard::new();
    let context = strict_session(99_999);
    let gc = Cli::try_parse_from(["agent-desktop", "session", "gc", "--ended"])
        .expect("gc parses")
        .command
        .expect("subcommand is present");

    let Err(AppError::Adapter(error)) = run(gc, &context) else {
        panic!("the command end must fail under a strict trace");
    };

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
}
