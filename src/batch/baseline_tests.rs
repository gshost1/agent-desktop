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

impl agent_desktop_core::ActionOps for AtomicEventAdapter {}

impl agent_desktop_core::InputOps for AtomicEventAdapter {
    fn clear_clipboard(
        &self,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::OperationReport, agent_desktop_core::AdapterError> {
        self.opened.store(true, Ordering::SeqCst);
        Ok(agent_desktop_core::OperationReport::delivered_unverified(
            "test",
            agent_desktop_core::OperationTarget::Clipboard,
        ))
    }
}

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
            {"command": "clipboard-clear", "args": {}},
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
}

impl agent_desktop_core::ObservationOps for NotifyOnClearAdapter {}

impl agent_desktop_core::ActionOps for NotifyOnClearAdapter {}

impl agent_desktop_core::InputOps for NotifyOnClearAdapter {
    fn clear_clipboard(
        &self,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::OperationReport, agent_desktop_core::AdapterError> {
        self.sent.store(true, Ordering::SeqCst);
        Ok(agent_desktop_core::OperationReport::delivered_unverified(
            "test",
            agent_desktop_core::OperationTarget::Clipboard,
        ))
    }
}

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
            {"command": "clipboard-clear", "args": {}},
            {
                "command": "wait",
                "args": {"notification": true, "app": "Fixture", "timeout": 300}
            }
        ])
        .to_string(),
        stop_on_error: true,
        timeout_ms: 60_000,
    };
    let adapter = NotifyOnClearAdapter {
        sent: AtomicBool::new(false),
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
    assert_eq!(value["results"][1]["data"]["notification"]["title"], "Sent");
}

struct UnreadableNotificationsAdapter {
    cleared: AtomicBool,
}

impl agent_desktop_core::ObservationOps for UnreadableNotificationsAdapter {}

impl agent_desktop_core::ActionOps for UnreadableNotificationsAdapter {}

impl agent_desktop_core::InputOps for UnreadableNotificationsAdapter {
    fn clear_clipboard(
        &self,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::OperationReport, agent_desktop_core::AdapterError> {
        self.cleared.store(true, Ordering::SeqCst);
        Ok(agent_desktop_core::OperationReport::delivered_unverified(
            "test",
            agent_desktop_core::OperationTarget::Clipboard,
        ))
    }
}

impl agent_desktop_core::SystemOps for UnreadableNotificationsAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::InteractionLease, agent_desktop_core::AdapterError> {
        agent_desktop_core::InteractionLease::guarded(deadline, ())
    }
}

#[test]
fn an_unreadable_notification_seed_stops_before_the_action() {
    let args = BatchArgs {
        commands_json: serde_json::json!([
            {"command": "clipboard-clear", "args": {}},
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

    assert!(!adapter.cleared.load(Ordering::SeqCst), "{value}");
    assert_eq!(value["stopped"]["reason"], "pre_action_baseline_failed");
    assert_eq!(
        value["results"][0]["error"]["disposition"]["delivery"], "not_delivered",
        "{value}"
    );
}
