use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{ActionOps, AdapterError, InputOps, ObservationOps, SystemOps};

use super::WebChannelAdapter;
use crate::channel::test_support::ScriptedConnector;

#[derive(Default)]
struct PreparingPlatform {
    prepared: AtomicUsize,
}

impl ObservationOps for PreparingPlatform {
    fn prepare_window_inventory(&self) -> Result<(), AdapterError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl ActionOps for PreparingPlatform {}
impl InputOps for PreparingPlatform {}
impl SystemOps for PreparingPlatform {}

#[test]
fn preparing_a_window_inventory_reaches_the_platform() {
    let web = WebChannelAdapter::with_connector(
        PreparingPlatform::default(),
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    assert!(web.prepare_window_inventory().is_ok());
    assert_eq!(web.inner().prepared.load(Ordering::SeqCst), 1);
}
