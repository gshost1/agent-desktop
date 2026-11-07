use super::*;
use std::sync::Mutex;

struct RefWindowAdapter {
    events: Mutex<Vec<&'static str>>,
    mismatch: bool,
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
        Ok(())
    }
}

#[test]
fn recovered_ref_window_is_strictly_verified_before_focus_without_rewriting_source() {
    for mismatch in [false, true] {
        let adapter = RefWindowAdapter {
            events: Mutex::new(Vec::new()),
            mismatch,
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
            assert_eq!(result.unwrap_err().code(), "STALE_REF");
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

    let crate::AppError::Adapter(focus) = focus else {
        panic!("expected an adapter error");
    };
    for error in [focus, saved, no_window] {
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
        assert_eq!(error.details.unwrap()["physical_delivery_started"], false);
    }
}
