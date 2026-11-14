use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{ActionOps, AdapterError, InputOps, ObservationOps, ProcessId, SystemOps};

use super::WebChannelAdapter;
use crate::channel::test_support::ScriptedConnector;

#[derive(Default)]
struct PreparingPlatform {
    prepared: AtomicUsize,
    connected: AtomicUsize,
}

impl ObservationOps for PreparingPlatform {
    fn prepare_window_inventory(&self) -> Result<(), AdapterError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn prepare_application_connection(&self, pid: ProcessId) -> Result<(), AdapterError> {
        self.connected.store(pid.get() as usize, Ordering::SeqCst);
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

#[test]
fn preparing_an_application_connection_reaches_the_platform() {
    let web = WebChannelAdapter::with_connector(
        PreparingPlatform::default(),
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    assert!(
        web.prepare_application_connection(ProcessId::new(42))
            .is_ok()
    );
    assert_eq!(web.inner().connected.load(Ordering::SeqCst), 42);
}
