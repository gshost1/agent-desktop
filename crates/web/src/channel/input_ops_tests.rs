use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{ActionOps, AdapterError, InputOps, ObservationOps, SystemOps};

use super::WebChannelAdapter;
use crate::channel::test_support::ScriptedConnector;

#[derive(Default)]
struct PreparingPlatform {
    prepared: AtomicUsize,
}

impl ObservationOps for PreparingPlatform {}
impl ActionOps for PreparingPlatform {}
impl InputOps for PreparingPlatform {
    fn prepare_keyboard_input(&self) -> Result<(), AdapterError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl SystemOps for PreparingPlatform {}

#[test]
fn preparing_keyboard_input_reaches_the_platform() {
    let web = WebChannelAdapter::with_connector(
        PreparingPlatform::default(),
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    assert!(web.prepare_keyboard_input().is_ok());
    assert_eq!(web.inner().prepared.load(Ordering::SeqCst), 1);
}

#[derive(Default)]
struct CoordinateOnlyPlatform;

impl ObservationOps for CoordinateOnlyPlatform {}
impl ActionOps for CoordinateOnlyPlatform {}
impl InputOps for CoordinateOnlyPlatform {
    fn physical_pointer_refusal(&self) -> Option<AdapterError> {
        Some(AdapterError::not_supported("element-aimed pointer"))
    }

    fn coordinate_pointer_refusal(&self) -> Option<AdapterError> {
        None
    }
}
impl SystemOps for CoordinateOnlyPlatform {}

#[test]
fn both_pointer_gates_answer_as_the_platform_does() {
    let web = WebChannelAdapter::with_connector(
        CoordinateOnlyPlatform,
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    assert!(web.physical_pointer_refusal().is_some());
    assert!(web.coordinate_pointer_refusal().is_none());
}
