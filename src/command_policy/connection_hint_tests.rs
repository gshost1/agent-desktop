use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{
    ActionOps, AdapterError, InputOps, ObservationOps, ProcessId, RefMap, SystemOps,
    refs::refs_store::RefStore,
};
use clap::Parser;

use super::prepare_connections;
use crate::cli::{Cli, Commands};
use crate::dispatch::test_support::HomeGuard;

#[derive(Default)]
struct PreparingAdapter {
    prepared: AtomicUsize,
    connected: std::sync::Mutex<Vec<u32>>,
    keyboards: AtomicUsize,
}

impl ObservationOps for PreparingAdapter {
    fn prepare_window_inventory(&self) -> Result<(), AdapterError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Err(AdapterError::not_supported("prepare_window_inventory"))
    }

    fn prepare_application_connection(&self, pid: ProcessId) -> Result<(), AdapterError> {
        if let Ok(mut connected) = self.connected.lock() {
            connected.push(pid.get());
        }
        Err(AdapterError::not_supported(
            "prepare_application_connection",
        ))
    }
}
impl ActionOps for PreparingAdapter {}
impl InputOps for PreparingAdapter {
    fn prepare_keyboard_input(&self) -> Result<(), AdapterError> {
        self.keyboards.fetch_add(1, Ordering::SeqCst);
        Err(AdapterError::not_supported("prepare_keyboard_input"))
    }
}
impl SystemOps for PreparingAdapter {}

fn command(arguments: &[&str]) -> Commands {
    let argv = std::iter::once("agent-desktop").chain(arguments.iter().copied());
    let Ok(cli) = Cli::try_parse_from(argv) else {
        panic!("{arguments:?} parses");
    };
    let Some(command) = cli.command else {
        panic!("{arguments:?} names a subcommand");
    };
    command
}

fn prepared(arguments: &[&str]) -> usize {
    let adapter = PreparingAdapter::default();
    prepare_connections(&command(arguments), &adapter, None);
    adapter.prepared.load(Ordering::SeqCst)
}

fn connected(arguments: &[&str], session_id: Option<&str>) -> (Vec<u32>, usize) {
    let adapter = PreparingAdapter::default();
    prepare_connections(&command(arguments), &adapter, session_id);
    let connected = adapter
        .connected
        .lock()
        .map(|connected| connected.clone())
        .unwrap_or_default();
    (connected, adapter.keyboards.load(Ordering::SeqCst))
}

fn saved_ref(session_id: Option<&str>, pid: u32) -> String {
    let entry = serde_json::json!({
        "pid": pid,
        "process_instance": "test-instance",
        "role": "button",
        "name": "Send",
        "capture": "unique",
        "available_actions": ["Click"],
        "path": [0]
    });
    let map: RefMap =
        serde_json::from_value(serde_json::json!({ "inner": { "@e1": entry }, "counter": 1 }))
            .unwrap();
    let snapshot_id = RefStore::for_session(session_id)
        .unwrap()
        .save_new_snapshot(&map)
        .unwrap();
    format!("@{snapshot_id}:e1")
}

#[test]
fn a_ref_command_opens_the_connection_to_the_process_its_ref_names() {
    let _home = HomeGuard::new();
    let ref_id = saved_ref(None, 4242);
    assert_eq!(connected(&["click", &ref_id], None), (vec![4242], 0));
    assert_eq!(
        connected(&["get", &ref_id, "--property", "text"], None),
        (vec![4242], 0)
    );
    assert_eq!(
        connected(&["wait", "--element", &ref_id], None),
        (vec![4242], 0)
    );
    assert_eq!(
        connected(&["press", "--ref", &ref_id, "escape"], None),
        (vec![4242], 0)
    );
}

#[test]
fn a_ref_from_another_session_or_no_snapshot_opens_nothing() {
    let _home = HomeGuard::new();
    let session_ref = saved_ref(Some("agent-a"), 4343);
    assert_eq!(
        connected(&["click", &session_ref], Some("agent-a")),
        (vec![4343], 0)
    );
    assert_eq!(connected(&["click", &session_ref], None), (vec![], 0));
    assert_eq!(connected(&["click", "@s8f3k2p9:e1"], None), (vec![], 0));
    assert_eq!(
        connected(&["press", "--app", "Calculator", "escape"], None),
        (vec![], 0)
    );
    assert_eq!(connected(&["list-windows"], None), (vec![], 0));
}

#[test]
fn commands_that_read_the_window_inventory_prepare_it_once() {
    assert_eq!(prepared(&["snapshot", "--app", "Calculator", "-i"]), 1);
    assert_eq!(
        prepared(&["find", "--app", "Calculator", "--role", "button"]),
        1
    );
    assert_eq!(
        prepared(&["wait", "--app", "Calculator", "--text", "Done"]),
        1
    );
    assert_eq!(prepared(&["press", "--app", "Calculator", "escape"]), 1);
    assert_eq!(prepared(&["press", "--ref", "@s8f3k2p9:e1", "escape"]), 1);
    assert_eq!(prepared(&["launch", "Calculator"]), 1);
}

#[test]
fn commands_that_never_list_windows_leave_the_connection_alone() {
    assert_eq!(prepared(&["click", "@s8f3k2p9:e1"]), 0);
    assert_eq!(prepared(&["get", "@s8f3k2p9:e1", "--property", "text"]), 0);
    assert_eq!(prepared(&["set-value", "@s8f3k2p9:e1", "x"]), 0);
    assert_eq!(prepared(&["wait", "500"]), 0);
    assert_eq!(prepared(&["list-windows"]), 0);
    assert_eq!(prepared(&["close-app", "Calculator"]), 0);
    assert_eq!(prepared(&["list-surfaces", "--app", "Calculator"]), 0);
}
