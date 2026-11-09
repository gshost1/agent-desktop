use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::system::launch_notes::LaunchNotes;
use crate::system::launch_options::LaunchOptions;
use crate::system::launch_path::LaunchPath;
use crate::system::launch_presentation::LaunchPresentation;
use crate::{AdapterError, InteractionLease, ProcessId};
use std::sync::Mutex;

struct PathAdapter {
    path: Option<LaunchPath>,
    received: Mutex<Option<LaunchOptions>>,
}

impl ObservationOps for PathAdapter {}
impl ActionOps for PathAdapter {}
impl InputOps for PathAdapter {}

impl SystemOps for PathAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }

    fn launch_app(
        &self,
        _id: &str,
        options: &LaunchOptions,
        _lease: &InteractionLease,
    ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
        *self.received.lock().expect("lock") = Some(options.clone());
        Ok(crate::system::launch_result::LaunchResult {
            app: "Fixture".into(),
            pid: ProcessId::new(42),
            process_instance: None,
            window: None,
            cdp: None,
            renderer: None,
            notes: LaunchNotes {
                suggestion: None,
                launch_path: self.path,
            },
        })
    }
}

fn run(path: Option<LaunchPath>, options: LaunchOptions) -> (serde_json::Value, LaunchOptions) {
    let adapter = PathAdapter {
        path,
        received: Mutex::new(None),
    };
    let value = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options,
            download_dir: None,
        },
        &adapter,
    )
    .expect("launch");
    let received = adapter.received.lock().expect("lock").clone();
    (value, received.expect("adapter called"))
}

#[test]
fn the_reopen_request_reaches_the_adapter_unchanged() {
    let (_, received) = run(
        None,
        LaunchOptions {
            presentation: LaunchPresentation {
                reopen: true,
                ..Default::default()
            },
            ..Default::default()
        },
    );

    assert!(received.presentation.reopen);
}

#[test]
fn a_launch_does_not_ask_for_a_reopen_by_default() {
    let (_, received) = run(None, LaunchOptions::default());

    assert!(!received.presentation.reopen);
}

#[test]
fn the_launch_path_is_reported_in_snake_case() {
    for (path, expected) in [
        (LaunchPath::AttachedRunning, "attached_running"),
        (LaunchPath::ActivatedRunning, "activated_running"),
        (LaunchPath::Launched, "launched"),
        (LaunchPath::Reopened, "reopened"),
    ] {
        let (value, _) = run(Some(path), LaunchOptions::default());

        assert_eq!(value["launch_path"], expected);
    }
}

#[test]
fn a_platform_with_one_route_omits_the_launch_path() {
    let (value, _) = run(None, LaunchOptions::default());

    assert!(value.get("launch_path").is_none());
}
