use super::*;
use std::sync::Mutex;

struct RefWindowAdapter {
    events: Mutex<Vec<&'static str>>,
    mismatch: bool,
    focus_fails: bool,
}

impl crate::ObservationOps for RefWindowAdapter {}
impl crate::ActionOps for RefWindowAdapter {}
impl crate::InputOps for RefWindowAdapter {}
impl crate::SystemOps for RefWindowAdapter {
    fn resolve_ref_focus_window(
        &self,
        entry: &crate::RefEntry,
        _: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.events.lock().unwrap().push("ref_window");
        assert!(entry.source.source_window_id.is_none());
        let mut evidence = entry.clone();
        evidence.source.source_window_id = Some("exact-live-owner".into());
        crate::saved_ref_focus_window(&evidence)
    }

    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.events.lock().unwrap().push("strict_window");
        assert_eq!(expected.id, "exact-live-owner");
        let mut live = expected.clone();
        if self.mismatch {
            live.process_instance = Some("different-generation".into());
        }
        Ok(live)
    }

    fn focus_window(
        &self,
        window: &WindowInfo,
        _: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.events.lock().unwrap().push("focus");
        assert_eq!(window.id, "exact-live-owner");
        if self.focus_fails {
            return Err(AdapterError::timeout("focus"));
        }
        Ok(())
    }

    fn focus_app(
        &self,
        _: crate::ProcessIdentity,
        _: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.events.lock().unwrap().push("focus_app");
        Ok(())
    }
}

#[test]
fn recovered_ref_window_is_strictly_verified_before_focus_without_rewriting_source() {
    for mismatch in [false, true] {
        let adapter = RefWindowAdapter {
            events: Mutex::new(Vec::new()),
            mismatch,
            focus_fails: false,
        };
        let mut entry = crate::adapter::minimal_ref_entry("menuitem", None);
        entry.source.source_surface = crate::SnapshotSurface::Menu;
        let original = entry.clone();
        let lease =
            crate::InteractionLease::guarded(crate::Deadline::standard().unwrap(), ()).unwrap();
        let result =
            focus_entry_window(&entry, &adapter, &crate::CommandContext::default(), &lease);
        assert_eq!(entry, original);
        if mismatch {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::StaleRef);
            assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
            assert!(
                error
                    .details
                    .is_none_or(|d| d.get("preparatory_focus").is_none())
            );
            assert_eq!(
                *adapter.events.lock().unwrap(),
                ["ref_window", "strict_window"]
            );
        } else {
            assert!(result.unwrap());
            assert_eq!(
                *adapter.events.lock().unwrap(),
                ["ref_window", "strict_window", "focus"]
            );
        }
    }
}

#[test]
fn a_headed_target_without_identity_is_refused_as_not_delivered() {
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.process.process_instance = None;
    let lease = crate::InteractionLease::guarded(crate::Deadline::standard().unwrap(), ()).unwrap();
    let adapter = crate::adapter::NoopAdapter;

    let focus = focus_entry_window(&entry, &adapter, &crate::CommandContext::default(), &lease)
        .unwrap_err();
    let saved = crate::saved_ref_focus_window(&entry).unwrap_err();
    let mut windowless = crate::adapter::minimal_ref_entry("button", None);
    windowless.source.source_window_id = None;
    let no_window = crate::saved_ref_focus_window(&windowless).unwrap_err();

    for error in [focus, saved, no_window] {
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
        assert_eq!(error.details.unwrap()["physical_delivery_started"], false);
    }
}

#[test]
fn a_failed_focus_call_is_not_delivered_and_records_the_focus() {
    let adapter = RefWindowAdapter {
        events: Mutex::new(Vec::new()),
        mismatch: false,
        focus_fails: true,
    };
    let mut entry = crate::adapter::minimal_ref_entry("menuitem", None);
    entry.source.source_surface = crate::SnapshotSurface::Menu;
    let lease = crate::InteractionLease::guarded(crate::Deadline::standard().unwrap(), ()).unwrap();

    let result = focus_entry_window(&entry, &adapter, &crate::CommandContext::default(), &lease);

    let error = result.unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert_eq!(
        error.details.unwrap()["preparatory_focus"],
        "may_have_moved_focus"
    );
}

#[test]
fn an_application_target_without_identity_is_refused_like_a_window_target() {
    let mut entry = crate::adapter::minimal_ref_entry("menuitem", None);
    entry.process.process_instance = None;
    let lease = crate::InteractionLease::guarded(crate::Deadline::standard().unwrap(), ()).unwrap();

    let error = focus_entry_app(&entry, &crate::adapter::NoopAdapter, &lease).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.message, MISSING_PROCESS_IDENTITY);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert!(!error.moved_focus());
}

#[test]
fn an_unproven_capture_is_refused_before_its_window_or_app_is_read_or_raised() {
    for capture in [
        crate::CaptureVerdict::Ambiguous,
        crate::CaptureVerdict::Incomplete,
        crate::CaptureVerdict::Unknown,
    ] {
        let adapter = RefWindowAdapter {
            events: Mutex::new(Vec::new()),
            mismatch: false,
            focus_fails: false,
        };
        let mut entry = crate::adapter::minimal_ref_entry("menuitem", None);
        entry.source.source_surface = crate::SnapshotSurface::Menu;
        entry.identity.capture = capture;
        let expected = crate::refs::resolve_errors::refused_before_reading(&entry).unwrap();
        let lease =
            crate::InteractionLease::guarded(crate::Deadline::standard().unwrap(), ()).unwrap();

        let window =
            focus_entry_window(&entry, &adapter, &crate::CommandContext::default(), &lease)
                .unwrap_err();
        let app = focus_entry_app(&entry, &adapter, &lease).unwrap_err();

        for error in [window, app] {
            assert_eq!(error.code, expected.code, "{capture:?}");
            assert_eq!(error.details, expected.details, "{capture:?}");
            assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
            assert!(!error.moved_focus());
        }
        assert!(adapter.events.lock().unwrap().is_empty(), "{capture:?}");
    }
}
