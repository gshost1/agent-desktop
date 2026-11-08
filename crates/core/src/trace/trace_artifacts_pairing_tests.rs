use super::tests::{MINI_PNG, artifacts_session, entry, setup_artifacts_test};
use crate::context::CommandContext;
use crate::refs::refs_store::RefStore;
use crate::{
    Action, ActionRequest, ActionResult, AdapterError, ErrorCode, ImageBuffer,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, ScreenshotTarget, SystemOps},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

struct HeldFlag(Arc<AtomicBool>);

impl Drop for HeldFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Refuses the first `refusals` resolutions under the lease with a retryable
/// `STALE_REF`, or refuses the lease itself, and counts every screenshot.
struct PairingAdapter {
    lease_fails: bool,
    refusals: AtomicU32,
    held: Arc<AtomicBool>,
    screenshots: AtomicU32,
    dispatches: AtomicU32,
}

impl PairingAdapter {
    fn new(lease_fails: bool, refusals: u32) -> Self {
        Self {
            lease_fails,
            refusals: AtomicU32::new(refusals),
            held: Arc::new(AtomicBool::new(false)),
            screenshots: AtomicU32::new(0),
            dispatches: AtomicU32::new(0),
        }
    }
}

impl ObservationOps for PairingAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let refuses = self.held.load(Ordering::SeqCst)
            && self
                .refusals
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                    left.checked_sub(1)
                })
                .is_ok();
        if refuses {
            return Err(
                AdapterError::new(ErrorCode::StaleRef, "moved under the lease")
                    .with_details(serde_json::json!({ "retryable": true })),
            );
        }
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!(
        "button",
        "Run",
        [crate::actions::capability::CLICK]
    );
    crate::adapter::saved_ref_scope!("button", Some("Run"));
}

impl ActionOps for PairingAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for PairingAdapter {}

impl SystemOps for PairingAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        if self.lease_fails {
            return Err(deadline
                .timeout_error()
                .with_details(serde_json::json!({ "kind": "interaction_process_lock_timeout" })));
        }
        self.held.store(true, Ordering::SeqCst);
        crate::InteractionLease::guarded(deadline, HeldFlag(Arc::clone(&self.held)))
    }

    fn screenshot(
        &self,
        _target: ScreenshotTarget,
        _deadline: crate::Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        self.screenshots.fetch_add(1, Ordering::SeqCst);
        Ok(ImageBuffer {
            data: MINI_PNG.to_vec(),
            format: crate::ImageFormat::Png,
            width: 1,
            height: 1,
            scale_factor: 1.0,
        })
    }
}

fn click(adapter: &PairingAdapter, context: &CommandContext) -> Result<ActionResult, AdapterError> {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    crate::ref_action::ref_action_wait::execute_with_auto_wait(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter,
            entry: &entry(42),
            ref_id: "@e1",
            context,
        },
        ActionRequest::headless(Action::Click).with_timeout_ms(Some(3_000)),
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
}

fn artifact_events(trace_dir: &std::path::Path) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    for entry in std::fs::read_dir(trace_dir).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_none_or(|extension| extension != "jsonl")
        {
            continue;
        }
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            if event["event"] == "action.artifacts" {
                events.push(event);
            }
        }
    }
    events
}

fn screen_files(trace_dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(trace_dir.join("screens"))
        .map(|dir| {
            dir.map(|entry| format!("screens/{}", entry.unwrap().file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default()
}

fn field<'a>(event: &'a serde_json::Value, key: &str) -> &'a serde_json::Value {
    event
        .get("fields")
        .map_or(&event[key], |fields| &fields[key])
}

#[test]
fn a_lease_failure_after_the_pre_screenshot_still_references_it() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    let adapter = PairingAdapter::new(true, 0);

    let error = click(&adapter, &context).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
    let files = screen_files(&trace_dir);
    assert_eq!(files.len(), 1, "one pre screenshot was written");
    let events = artifact_events(&trace_dir);
    assert_eq!(
        events.len(),
        1,
        "the pre screenshot has no event: {events:?}"
    );
    assert_eq!(field(&events[0], "screenshot_pre"), files[0].as_str());
    assert_eq!(field(&events[0], "skipped_post"), "not_dispatched");
}

#[test]
fn two_auto_wait_refusals_take_one_pre_screenshot() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    let adapter = PairingAdapter::new(false, 2);

    click(&adapter, &context).expect("the third attempt dispatches");

    assert_eq!(adapter.refusals.load(Ordering::SeqCst), 0);
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(
        adapter.screenshots.load(Ordering::SeqCst),
        2,
        "one pre and one post screenshot per command"
    );
    let events = artifact_events(&trace_dir);
    assert_eq!(
        events.len(),
        1,
        "one artifact event per command: {events:?}"
    );
    let files = screen_files(&trace_dir);
    assert_eq!(files.len(), 2);
    for key in ["screenshot_pre", "screenshot_post"] {
        let path = field(&events[0], key).as_str().unwrap_or_default();
        assert!(files.iter().any(|file| file == path), "{key} {path:?}");
    }
}
