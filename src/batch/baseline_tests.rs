use std::sync::atomic::{AtomicBool, Ordering};

use agent_desktop_core::{
    PermissionReport, SignalBaseline, SignalCompleteness, SignalFilter, WindowState,
};

use super::execute;
use crate::cli_args::batch::BatchArgs;

struct AtomicEventAdapter {
    opened: AtomicBool,
}

impl agent_desktop_core::ObservationOps for AtomicEventAdapter {
    fn list_apps(
        &self,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<Vec<agent_desktop_core::AppInfo>, agent_desktop_core::AdapterError> {
        Ok(vec![agent_desktop_core::AppInfo {
            name: "Fixture".into(),
            pid: agent_desktop_core::ProcessId::new(42),
            bundle_id: Some("com.example.fixture".into()),
            process_instance: Some("fixture-42".into()),
            presentation: None,
        }])
    }
}

impl agent_desktop_core::ActionOps for AtomicEventAdapter {
    fn execute_action(
        &self,
        _handle: &agent_desktop_core::NativeHandle,
        request: agent_desktop_core::ActionRequest,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::ActionResult, agent_desktop_core::AdapterError> {
        self.opened.store(true, Ordering::SeqCst);
        Ok(agent_desktop_core::ActionResult::from_execution(
            &request.action,
            Vec::new(),
        ))
    }
}

impl agent_desktop_core::InputOps for AtomicEventAdapter {}

impl agent_desktop_core::SystemOps for AtomicEventAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::InteractionLease, agent_desktop_core::AdapterError> {
        agent_desktop_core::InteractionLease::guarded(deadline, ())
    }

    fn capture_signal_baseline(
        &self,
        _filter: &SignalFilter,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<SignalBaseline, agent_desktop_core::AdapterError> {
        let windows = self
            .opened
            .load(Ordering::SeqCst)
            .then(|| agent_desktop_core::WindowInfo {
                id: "w-sync".into(),
                title: "Synchronous".into(),
                app: "Fixture".into(),
                pid: agent_desktop_core::ProcessId::new(42),
                process_instance: Some("fixture-42".into()),
                bounds: None,
                state: WindowState {
                    is_focused: true,
                    ..WindowState::default()
                },
            })
            .into_iter()
            .collect();
        Ok(SignalBaseline {
            windows,
            apps: Vec::new(),
            surfaces: Vec::new(),
            completeness: SignalCompleteness::complete(),
        })
    }
}

#[test]
fn action_then_event_wait_uses_a_pre_action_baseline() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "press", "args": {"combo": "a"}},
            {
                "command": "wait",
                "args": {"event": "window-opened", "app": "Fixture", "timeout": 100}
            }
        ])
        .to_string(),
        stop_on_error: true,
        timeout_ms: 60_000,
    };
    let adapter = AtomicEventAdapter {
        opened: AtomicBool::new(false),
    };

    let value = execute(
        args,
        &adapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["results"][0]["ok"], true, "{value}");
    assert_eq!(value["results"][1]["ok"], true, "{value}");
    assert_eq!(value["results"][1]["data"]["event"]["window_id"], "w-sync");
}

#[test]
fn an_event_wait_behind_a_read_only_entry_uses_a_baseline_from_before_the_action() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "press", "args": {"combo": "a"}},
            {"command": "version", "args": {}},
            {
                "command": "wait",
                "args": {"event": "window-opened", "app": "Fixture", "timeout": 100}
            }
        ])
        .to_string(),
        stop_on_error: true,
        timeout_ms: 60_000,
    };
    let adapter = AtomicEventAdapter {
        opened: AtomicBool::new(false),
    };

    let value = execute(
        args,
        &adapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["results"][1]["ok"], true, "{value}");
    assert_eq!(value["results"][2]["ok"], true, "{value}");
    assert_eq!(value["results"][2]["data"]["event"]["window_id"], "w-sync");
}

#[test]
fn wait_as_first_entry_runs_with_event_baseline_none() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {
                "command": "wait",
                "args": {"event": "window-opened", "app": "Fixture", "timeout": 100}
            },
            {"command": "version", "args": {}}
        ])
        .to_string(),
        stop_on_error: false,
        timeout_ms: 60_000,
    };
    let adapter = AtomicEventAdapter {
        opened: AtomicBool::new(false),
    };

    let value = execute(
        args,
        &adapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["results"][0]["execution"], "completed", "{value}");
    assert_eq!(value["results"][0]["ok"], false, "{value}");
    assert_eq!(value["results"][0]["error"]["code"], "TIMEOUT", "{value}");
    assert_eq!(value["results"][1]["execution"], "completed", "{value}");
    assert_eq!(value["results"][1]["ok"], true, "{value}");
    assert!(value.get("stopped").is_none(), "{value}");
}

struct NotifyOnClearAdapter {
    sent: AtomicBool,
    early_lists: std::sync::atomic::AtomicUsize,
}

fn notify_on_clear() -> NotifyOnClearAdapter {
    NotifyOnClearAdapter {
        sent: AtomicBool::new(false),
        early_lists: std::sync::atomic::AtomicUsize::new(0),
    }
}

impl agent_desktop_core::ObservationOps for NotifyOnClearAdapter {}

impl agent_desktop_core::ActionOps for NotifyOnClearAdapter {
    fn execute_action(
        &self,
        _handle: &agent_desktop_core::NativeHandle,
        request: agent_desktop_core::ActionRequest,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::ActionResult, agent_desktop_core::AdapterError> {
        self.sent.store(true, Ordering::SeqCst);
        Ok(agent_desktop_core::ActionResult::from_execution(
            &request.action,
            Vec::new(),
        ))
    }
}

impl agent_desktop_core::InputOps for NotifyOnClearAdapter {}

impl agent_desktop_core::SystemOps for NotifyOnClearAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::InteractionLease, agent_desktop_core::AdapterError> {
        agent_desktop_core::InteractionLease::guarded(deadline, ())
    }

    fn list_notifications(
        &self,
        _filter: &agent_desktop_core::NotificationFilter,
        _policy: agent_desktop_core::InteractionPolicy,
        _deadline: agent_desktop_core::Deadline,
        _lease: Option<&agent_desktop_core::InteractionLease>,
    ) -> Result<Vec<agent_desktop_core::NotificationInfo>, agent_desktop_core::AdapterError> {
        if !self.sent.load(Ordering::SeqCst) {
            self.early_lists.fetch_add(1, Ordering::SeqCst);
        }
        Ok(self
            .sent
            .load(Ordering::SeqCst)
            .then(|| agent_desktop_core::NotificationInfo {
                index: 1,
                app_name: "Fixture".into(),
                title: "Sent".into(),
                body: None,
                actions: Vec::new(),
            })
            .into_iter()
            .collect())
    }
}

#[test]
fn action_then_notification_wait_uses_a_pre_action_baseline() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "press", "args": {"combo": "a"}},
            {
                "command": "wait",
                "args": {"notification": true, "app": "Fixture", "timeout": 300}
            }
        ])
        .to_string(),
        stop_on_error: true,
        timeout_ms: 60_000,
    };
    let adapter = notify_on_clear();

    let value = execute(
        args,
        &adapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["results"][0]["ok"], true, "{value}");
    assert_eq!(value["results"][1]["ok"], true, "{value}");
    assert_eq!(value["results"][1]["data"]["notification"]["title"], "Sent");
}

struct UnreadableNotificationsAdapter {
    cleared: AtomicBool,
}

impl agent_desktop_core::ObservationOps for UnreadableNotificationsAdapter {}

impl agent_desktop_core::ActionOps for UnreadableNotificationsAdapter {
    fn execute_action(
        &self,
        _handle: &agent_desktop_core::NativeHandle,
        request: agent_desktop_core::ActionRequest,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::ActionResult, agent_desktop_core::AdapterError> {
        self.cleared.store(true, Ordering::SeqCst);
        Ok(agent_desktop_core::ActionResult::from_execution(
            &request.action,
            Vec::new(),
        ))
    }
}

impl agent_desktop_core::InputOps for UnreadableNotificationsAdapter {}

impl agent_desktop_core::SystemOps for UnreadableNotificationsAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::InteractionLease, agent_desktop_core::AdapterError> {
        agent_desktop_core::InteractionLease::guarded(deadline, ())
    }
}

#[test]
fn an_unreadable_notification_seed_does_not_block_the_action() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "press", "args": {"combo": "a"}},
            {"command": "wait", "args": {"notification": true, "timeout": 100}}
        ])
        .to_string(),
        stop_on_error: false,
        timeout_ms: 60_000,
    };
    let adapter = UnreadableNotificationsAdapter {
        cleared: AtomicBool::new(false),
    };

    let value = execute(
        args,
        &adapter,
        &PermissionReport::default(),
        &agent_desktop_core::CommandContext::default(),
    )
    .unwrap();

    assert!(adapter.cleared.load(Ordering::SeqCst), "{value}");
    assert!(value.get("stopped").is_none(), "{value}");
    assert_eq!(value["results"][0]["ok"], true, "{value}");
    assert_eq!(
        value["results"][1]["error"]["details"]["pre_action_seed"], "unavailable",
        "{value}"
    );
}

fn press_then_wait(
    wait_args: serde_json::Value,
    context: agent_desktop_core::CommandContext,
) -> (serde_json::Value, usize) {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "press", "args": {"combo": "a"}},
            {"command": "wait", "args": wait_args}
        ])
        .to_string(),
        stop_on_error: false,
        timeout_ms: 60_000,
    };
    let adapter = notify_on_clear();
    let value = execute(args, &adapter, &PermissionReport::default(), &context).unwrap();
    (value, adapter.early_lists.load(Ordering::SeqCst))
}

#[test]
fn a_headed_notification_wait_is_not_seeded_before_the_action() {
    let (value, early_lists) = press_then_wait(
        serde_json::json!({"notification": true, "app": "Fixture", "timeout": 200}),
        agent_desktop_core::CommandContext::default().with_headed(true),
    );

    assert_eq!(early_lists, 0, "{value}");
    assert_eq!(value["results"][0]["ok"], true, "{value}");
    assert_eq!(
        value["results"][1]["error"]["details"]["pre_action_seed"], "skipped_headed",
        "{value}"
    );
}

#[test]
fn a_wait_that_does_not_validate_as_a_notification_wait_is_not_seeded() {
    let (value, early_lists) = press_then_wait(
        serde_json::json!({"notification": true, "ms": 5, "timeout": 200}),
        agent_desktop_core::CommandContext::default(),
    );

    assert_eq!(early_lists, 0, "{value}");
    assert_eq!(value["results"][0]["ok"], true, "{value}");
    assert_eq!(value["results"][1]["ok"], false, "{value}");
}
