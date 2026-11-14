use serde::{Deserialize, Serialize};

use crate::{ProcessId, SnapshotOwner};

/// What a snapshot observed: the process, by pid and process instance, and
/// the window inside it when the snapshot was window-owned. Two snapshots are
/// compared only when they observed the same owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DiffOwner {
    pub(crate) pid: ProcessId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) process_instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) window_id: Option<String>,
}

impl DiffOwner {
    pub(crate) fn of(owner: &SnapshotOwner) -> Self {
        match owner {
            SnapshotOwner::Application(app) => Self {
                pid: app.pid,
                process_instance: app.process_instance.clone(),
                window_id: None,
            },
            SnapshotOwner::Window(window) => Self {
                pid: window.pid,
                process_instance: window.process_instance.clone(),
                window_id: owner.window().map(|window| window.id.clone()),
            },
        }
    }
}
