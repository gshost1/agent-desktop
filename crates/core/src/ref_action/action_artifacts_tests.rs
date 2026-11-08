use crate::context::CommandContext;
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{ArtifactsMode, SessionTraceMode, StartSessionOptions, start_session};
use crate::wait::deadline::VirtualClock;
use crate::{
    Action, ActionRequest, ActionResult, AdapterError, DeliverySemantics, ErrorCode, ImageBuffer,
    RefEntry,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, ScreenshotTarget, SystemOps},
};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Plain,
    LeaseFails,
    SlowCapture,
    FirstCaptureTimesOut,
    Focuses,
    DispatchFails(DeliverySemantics),
}

struct HeldFlag(Arc<AtomicBool>);

impl Drop for HeldFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Refuses the first `refusals` resolutions under the lease with a retryable
/// `STALE_REF`, and counts every screenshot and dispatch.
struct PairingAdapter {
    mode: Mode,
    refusals: AtomicU32,
    held: Arc<AtomicBool>,
    screenshots: AtomicU32,
    dispatches: AtomicU32,
}

impl PairingAdapter {
    fn new(mode: Mode, refusals: u32) -> Self {
        Self {
            mode,
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
        _entry: &RefEntry,
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
    fn headed_requirement_for_ref(
        &self,
        _entry: &RefEntry,
        request: &ActionRequest,
        _deadline: crate::Deadline,
    ) -> Result<crate::HeadedRequirement, AdapterError> {
        Ok(match self.mode {
            Mode::Focuses => crate::HeadedRequirement::FocusedApplication,
            _ => request.headed_requirement(),
        })
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        if let Mode::DispatchFails(disposition) = self.mode {
            return Err(AdapterError::internal("dispatch failed").with_disposition(disposition));
        }
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for PairingAdapter {}

impl SystemOps for PairingAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        if self.mode == Mode::LeaseFails {
            return Err(deadline
                .timeout_error()
                .with_details(serde_json::json!({ "kind": "interaction_process_lock_timeout" })));
        }
        self.held.store(true, Ordering::SeqCst);
        crate::InteractionLease::guarded(deadline, HeldFlag(Arc::clone(&self.held)))
    }

    fn focus_app(
        &self,
        _process: crate::ProcessIdentity,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    fn screenshot(
        &self,
        _target: ScreenshotTarget,
        deadline: crate::Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        let call = self.screenshots.fetch_add(1, Ordering::SeqCst);
        let slow = self.mode == Mode::SlowCapture
            || (self.mode == Mode::FirstCaptureTimesOut && call == 0);
        if slow {
            crate::wait::deadline::sleep(deadline.remaining());
            return Err(deadline.timeout_error());
        }
        Ok(ImageBuffer {
            data: PNG.to_vec(),
            format: crate::ImageFormat::Png,
            width: 1,
            height: 1,
            scale_factor: 1.0,
        })
    }
}

fn entry() -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    entry.process = crate::RefProcess {
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
    };
    entry.source.source_window_id = Some("w-42".into());
    entry.capabilities.available_actions = vec![crate::actions::capability::CLICK.into()];
    entry
}

/// A full-artifacts session in a throwaway home, and the trace directory its
/// artifacts land in.
fn session() -> (HomeGuard, CommandContext, std::path::PathBuf) {
    let home = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        artifacts: ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    (home, context, trace_dir)
}

fn click(
    adapter: &PairingAdapter,
    context: &CommandContext,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    crate::ref_action::ref_action_wait::execute_with_auto_wait(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter,
            entry: &entry(),
            ref_id: "@e1",
            context,
        },
        request,
        crate::ref_action::ref_action_single::dispatch_resolved,
    )
}

fn headless(timeout_ms: u64) -> ActionRequest {
    ActionRequest::headless(Action::Click).with_timeout_ms(Some(timeout_ms))
}

fn artifact_events(trace_dir: &std::path::Path) -> Vec<Value> {
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
            let event: Value = serde_json::from_str(line).unwrap();
            if event["event"] == "action.artifacts" {
                events.push(event.get("fields").cloned().unwrap_or(event));
            }
        }
    }
    events
}

fn only_event(trace_dir: &std::path::Path) -> Value {
    let events = artifact_events(trace_dir);
    assert_eq!(
        events.len(),
        1,
        "one artifact event per command: {events:?}"
    );
    events[0].clone()
}

fn screen_files(trace_dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(trace_dir.join("screens"))
        .map(|dir| {
            dir.map(|item| format!("screens/{}", item.unwrap().file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_lease_failure_after_the_pre_screenshot_still_references_it() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::LeaseFails, 0);

    let error = click(&adapter, &context, headless(3_000)).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
    let files = screen_files(&trace_dir);
    assert_eq!(files.len(), 1, "one pre screenshot was written");
    let event = only_event(&trace_dir);
    assert_eq!(event["screenshot_pre"], files[0].as_str());
    assert_eq!(event["skipped_post"], "not_dispatched");
}

#[test]
fn two_auto_wait_refusals_take_one_pre_screenshot_and_record_its_age() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::Plain, 2);

    click(&adapter, &context, headless(3_000)).expect("the third attempt dispatches");

    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.screenshots.load(Ordering::SeqCst), 2);
    let event = only_event(&trace_dir);
    let files = screen_files(&trace_dir);
    for key in ["screenshot_pre", "screenshot_post"] {
        let path = event[key].as_str().unwrap_or_default();
        assert!(files.iter().any(|file| file == path), "{key} {path:?}");
    }
    let poll_ms = crate::refs::ref_resolve_deadline::POLL_INTERVAL.as_millis() as u64;
    let age = event["screenshot_pre_age_ms"].as_u64();
    assert!(
        age.is_some_and(|age| age >= 2 * poll_ms),
        "the pre screenshot predates two refusals: {event}"
    );
}

#[test]
fn the_post_screenshot_spends_only_what_the_action_budget_has_left() {
    let (_home, context, trace_dir) = session();
    let clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::SlowCapture, 0);

    click(&adapter, &context, headless(200)).expect("the click dispatches");

    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
    assert!(
        clock.elapsed() <= Duration::from_millis(200),
        "a 200 ms action ran for {:?}",
        clock.elapsed()
    );
    let event = only_event(&trace_dir);
    assert!(
        event["skipped_post"].is_string() || event["skipped"].is_string(),
        "{event}"
    );
}

#[test]
fn a_pre_screenshot_that_ran_out_of_time_is_retried_on_the_next_attempt() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::FirstCaptureTimesOut, 1);

    click(&adapter, &context, headless(5_000)).expect("the second attempt dispatches");

    let event = only_event(&trace_dir);
    assert!(event["screenshot_pre"].is_string(), "{event}");
    assert!(event.get("skipped_pre").is_none(), "{event}");
    assert!(event["screenshot_post"].is_string(), "{event}");
}

#[test]
fn a_refusal_after_a_focus_call_records_the_post_screen_and_no_dispatch() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::Focuses, 1);
    let request = ActionRequest::headed(Action::Click).with_timeout_ms(Some(3_000));

    let error = click(&adapter, &context, request).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 0);
    let event = only_event(&trace_dir);
    assert!(event["screenshot_post"].is_string(), "{event}");
    assert_eq!(event["delivery"], "not_delivered");
    assert!(event.get("skipped_post").is_none(), "{event}");
}

#[test]
fn an_unknown_dispatch_failure_records_the_post_screen_and_its_disposition() {
    let (_home, context, trace_dir) = session();
    let _clock = VirtualClock::freeze();
    let adapter = PairingAdapter::new(Mode::DispatchFails(DeliverySemantics::unknown()), 0);

    let error = click(&adapter, &context, headless(3_000)).unwrap_err();

    assert_eq!(error.disposition, DeliverySemantics::unknown());
    let event = only_event(&trace_dir);
    assert!(event["screenshot_post"].is_string(), "{event}");
    assert_eq!(event["delivery"], "unknown");
}

#[test]
fn a_resolved_dispatch_that_delivered_nothing_takes_no_post_screenshot() {
    let (_home, context, trace_dir) = session();
    let adapter = PairingAdapter::new(Mode::DispatchFails(DeliverySemantics::not_delivered()), 0);
    let target_entry = entry();
    let deadline = crate::Deadline::standard().unwrap();
    let lease = crate::adapter::SystemOps::acquire_interaction_lease(&adapter, deadline).unwrap();
    let handle = NativeHandle::null();
    let target = crate::ref_action::ref_action_context::RefActionContext::new(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter: &adapter,
            entry: &target_entry,
            ref_id: "@e1",
            context: &context,
        },
        deadline,
    );

    let result = crate::ref_action::execute_resolved(
        crate::ref_action::ResolvedRefAction::new(target, &handle),
        ActionRequest::headless(Action::Click),
        &lease,
    );

    assert!(result.is_err());
    assert_eq!(adapter.screenshots.load(Ordering::SeqCst), 1);
    assert_eq!(only_event(&trace_dir)["skipped_post"], "not_dispatched");
}
