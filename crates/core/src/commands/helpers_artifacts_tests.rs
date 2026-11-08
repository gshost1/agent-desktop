use super::test_support::entry;
use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::context::WaitSelector;
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::refs::RefMap;
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{ArtifactsMode, SessionTraceMode, StartSessionOptions, start_session};
use crate::{AdapterError, ImageBuffer, WindowInfo};
use crate::{actions::action::Action, actions::action_result::ActionResult};
use std::sync::Mutex;
use std::time::Duration;

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Records the order of dispatch, screenshots and post-action wait polls. A
/// wait poll takes six seconds, longer than the action's own budget.
#[derive(Default)]
struct OrderAdapter {
    log: Mutex<Vec<&'static str>>,
}

impl OrderAdapter {
    fn dispatched(&self) -> bool {
        self.log.lock().unwrap().contains(&"dispatch")
    }

    fn window() -> WindowInfo {
        WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TargetApp".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }
    }
}

impl ObservationOps for OrderAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Element { .. }) {
            return crate::adapter::saved_ref_element_tree(root);
        }
        if self.dispatched() {
            self.log.lock().unwrap().push("wait_poll");
            crate::wait::deadline::sleep(Duration::from_secs(6));
        }
        let mut window = crate::adapter::minimal_accessibility_node("window", "Saved!");
        window.children = vec![crate::adapter::minimal_accessibility_node("button", "OK")];
        crate::adapter::observed_tree(&root, window)
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![Self::window()])
    }

    crate::adapter::complete_live_observation!("button", "OK", [crate::actions::capability::CLICK]);
    crate::adapter::saved_ref_anchor!();
}

impl ActionOps for OrderAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.log.lock().unwrap().push("dispatch");
        Ok(ActionResult::delivered_unverified("ok"))
    }
}

impl InputOps for OrderAdapter {}

impl SystemOps for OrderAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(expected.clone())
    }

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<crate::system::process_state::ProcessState, AdapterError> {
        Ok(crate::system::process_state::ProcessState::Running)
    }

    fn screenshot(
        &self,
        _target: crate::adapter::ScreenshotTarget,
        _deadline: crate::Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        self.log.lock().unwrap().push("screenshot");
        Ok(ImageBuffer {
            data: PNG.to_vec(),
            format: crate::ImageFormat::Png,
            width: 1,
            height: 1,
            scale_factor: 1.0,
        })
    }
}

fn artifact_fields(trace_dir: &std::path::Path) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    for item in std::fs::read_dir(trace_dir).unwrap() {
        let path = item.unwrap().path();
        if path
            .extension()
            .is_none_or(|extension| extension != "jsonl")
        {
            continue;
        }
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            if event["event"] == "action.artifacts" {
                events.push(event.get("fields").cloned().unwrap_or(event));
            }
        }
    }
    events
}

#[test]
fn wait_for_takes_the_post_screenshot_right_after_dispatch() {
    let _home = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        artifacts: ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    let store = RefStore::for_session(Some(&manifest.id)).unwrap();
    let mut target = entry();
    target.source.source_app = Some("TargetApp".into());
    target.source.source_window_id = Some("w-1".into());
    let mut refmap = RefMap::new();
    refmap.allocate(target);
    let snapshot_id = store.save_new_snapshot(&refmap).unwrap();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false)
        .unwrap()
        .with_wait_selector(Some(WaitSelector {
            query_raw: ":saved!".into(),
            gone: false,
            timeout_ms: 20_000,
        }));
    let adapter = OrderAdapter::default();
    let _clock = crate::wait::deadline::VirtualClock::freeze();

    let value = execute_ref_action_with_context(
        RefArgs {
            ref_id: format!("@{snapshot_id}:e1"),
            timeout_ms: None,
        },
        &adapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .expect("the click and its wait succeed");

    assert_eq!(value["matched_selector"], ":saved!");
    let log = adapter.log.lock().unwrap().clone();
    let dispatch = log.iter().position(|step| *step == "dispatch").unwrap();
    assert_eq!(
        log.get(dispatch + 1).copied(),
        Some("screenshot"),
        "the post screenshot must follow the dispatch, before the wait: {log:?}"
    );
    assert!(log[dispatch..].contains(&"wait_poll"), "{log:?}");
    let events = artifact_fields(&store.trace_dir());
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(
        events[0]["screenshot_post"]
            .as_str()
            .is_some_and(|path| path.starts_with("screens/")),
        "the post screenshot was not recorded: {events:?}"
    );
}
