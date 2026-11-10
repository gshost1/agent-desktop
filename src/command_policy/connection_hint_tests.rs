use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{ActionOps, AdapterError, InputOps, ObservationOps, SystemOps};
use clap::Parser;

use super::prepare_connections;
use crate::cli::{Cli, Commands};

#[derive(Default)]
struct PreparingAdapter {
    prepared: AtomicUsize,
}

impl ObservationOps for PreparingAdapter {
    fn prepare_window_inventory(&self) -> Result<(), AdapterError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Err(AdapterError::not_supported("prepare_window_inventory"))
    }
}
impl ActionOps for PreparingAdapter {}
impl InputOps for PreparingAdapter {}
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
    prepare_connections(&command(arguments), &adapter);
    adapter.prepared.load(Ordering::SeqCst)
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
    assert_eq!(prepared(&["launch", "Calculator"]), 1);
}

#[test]
fn commands_that_never_list_windows_leave_the_connection_alone() {
    assert_eq!(prepared(&["click", "@s8f3k2p9:e1"]), 0);
    assert_eq!(prepared(&["get", "@s8f3k2p9:e1", "--property", "text"]), 0);
    assert_eq!(prepared(&["set-value", "@s8f3k2p9:e1", "x"]), 0);
    assert_eq!(prepared(&["press", "--ref", "@s8f3k2p9:e1", "escape"]), 0);
    assert_eq!(prepared(&["wait", "500"]), 0);
    assert_eq!(prepared(&["list-windows"]), 0);
    assert_eq!(prepared(&["close-app", "Calculator"]), 0);
    assert_eq!(prepared(&["list-surfaces", "--app", "Calculator"]), 0);
}
