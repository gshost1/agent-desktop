use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use agent_desktop_core::{
    AdapterError, AppInfo, CommandContext, Deadline, InteractionLease, MenuWait, MouseEvent,
    OperationReport, OperationTarget, PermissionReport, ProcessId, ProcessIdentity,
};

use super::execute;
use crate::cli_args::batch::BatchArgs;

/// A menu the action changes: `before` is what a read sees until the action
/// runs, `after` the reads the wait polls once it has run.
struct ActedMenu {
    before: bool,
    after: Mutex<VecDeque<bool>>,
    acted: Mutex<bool>,
    wait_reads: Mutex<usize>,
}

impl ActedMenu {
    fn new(before: bool, after: &[bool]) -> Self {
        Self {
            before,
            after: Mutex::new(after.iter().copied().collect()),
            acted: Mutex::new(false),
            wait_reads: Mutex::new(0),
        }
    }

    fn act(&self) -> Result<OperationReport, AdapterError> {
        *self.acted.lock().unwrap() = true;
        Ok(OperationReport::delivered_unverified(
            "test",
            OperationTarget::Clipboard,
        ))
    }

    fn read_after(&self) -> bool {
        *self.wait_reads.lock().unwrap() += 1;
        let mut after = self.after.lock().unwrap();
        match after.len() {
            0 => false,
            1 => after[0],
            _ => after.pop_front().unwrap_or(false),
        }
    }
}

impl agent_desktop_core::ObservationOps for ActedMenu {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "Fixture".into(),
            pid: ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("fixture-42".into()),
            presentation: None,
        }])
    }
}

impl agent_desktop_core::ActionOps for ActedMenu {}

impl agent_desktop_core::InputOps for ActedMenu {
    fn clear_clipboard(&self, _lease: &InteractionLease) -> Result<OperationReport, AdapterError> {
        self.act()
    }

    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.act()
    }
}

impl agent_desktop_core::SystemOps for ActedMenu {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }

    fn menu_open(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        assert!(
            !*self.acted.lock().unwrap(),
            "the seed is read before the action"
        );
        Ok(self.before)
    }

    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        mut wait: MenuWait,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        loop {
            let read_started = Instant::now();
            if wait.observe(self.read_after(), read_started) {
                return Ok(());
            }
            if deadline.is_expired() {
                return Err(deadline.timeout_error());
            }
            std::thread::sleep(deadline.remaining_slice(Duration::from_millis(10))?);
        }
    }
}

fn run(action: serde_json::Value, adapter: &ActedMenu) -> serde_json::Value {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            action,
            {"command": "wait", "args": {"menu_closed": true, "app": "Fixture", "timeout": 5000}}
        ])
        .to_string(),
        stop_on_error: true,
        timeout_ms: 60_000,
    };
    execute(
        args,
        adapter,
        &PermissionReport::default(),
        &CommandContext::default().with_headed(true),
    )
    .unwrap()
}

fn actions() -> [serde_json::Value; 2] {
    [
        serde_json::json!({"command": "mouse-click", "args": {"xy": "10,10"}}),
        serde_json::json!({"command": "clipboard-clear", "args": {}}),
    ]
}

#[test]
fn a_menu_open_before_the_action_ends_the_closed_wait_without_the_grace() {
    for action in actions() {
        let adapter = ActedMenu::new(true, &[false]);
        let started = Instant::now();
        let value = run(action.clone(), &adapter);
        assert_eq!(value["results"][0]["ok"], true, "{value}");
        assert_eq!(value["results"][1]["ok"], true, "{value}");
        assert_eq!(*adapter.wait_reads.lock().unwrap(), 2, "{action}");
        assert!(
            started.elapsed() < Duration::from_millis(800),
            "{action}: {:?}",
            started.elapsed()
        );
    }
}

#[test]
fn an_opener_of_any_kind_holds_the_closed_wait_until_its_late_menu_closes() {
    for action in actions() {
        let adapter = ActedMenu::new(false, &[false, false, true, false]);
        let value = run(action.clone(), &adapter);
        assert_eq!(value["results"][1]["ok"], true, "{value}");
        assert_eq!(*adapter.wait_reads.lock().unwrap(), 5, "{action}");
    }
}
