use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{
    ActionOps, AdapterError, Deadline, InputOps, ObservationOps, ProcessId, SystemOps,
};

use super::WebChannelAdapter;
use crate::channel::test_support::ScriptedConnector;

#[derive(Default)]
struct PreparingPlatform {
    prepared: AtomicUsize,
    connected: AtomicUsize,
}

impl ObservationOps for PreparingPlatform {
    fn get_live_element_provisional(
        &self,
        _: &agent_desktop_core::NativeHandle,
        _: Deadline,
    ) -> Result<Option<agent_desktop_core::LiveElement>, AdapterError> {
        self.connected.fetch_add(1, Ordering::SeqCst);
        Err(AdapterError::not_supported("native provisional read"))
    }

    fn prepare_window_inventory(&self) -> Result<(), AdapterError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn prepare_application_connection(&self, pid: ProcessId) -> Result<(), AdapterError> {
        self.connected.store(pid.get() as usize, Ordering::SeqCst);
        Ok(())
    }

    fn is_auxiliary_window(&self, window: &agent_desktop_core::WindowInfo, _: Deadline) -> bool {
        window.id == "w-2"
    }

    fn menu_opener_grace(&self) -> std::time::Duration {
        std::time::Duration::from_millis(300)
    }
}
impl ActionOps for PreparingPlatform {}
impl InputOps for PreparingPlatform {}
impl SystemOps for PreparingPlatform {}

#[test]
fn provisional_live_read_forwards_native_handles_and_declines_web_handles() {
    let web = WebChannelAdapter::with_connector(
        PreparingPlatform::default(),
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    let deadline = Deadline::after(1000).unwrap();
    let native = agent_desktop_core::NativeHandle::null();
    assert!(web.get_live_element_provisional(&native, deadline).is_err());
    assert_eq!(web.inner().connected.load(Ordering::SeqCst), 1);
    let handle = agent_desktop_core::NativeHandle::new(crate::channel::web_handle::WebHandle {
        process: crate::channel::test_support::process(),
        token: crate::ElementToken::parse("cdp:page:frame:loader:1").unwrap(),
    });
    assert!(
        web.get_live_element_provisional(&handle, deadline)
            .unwrap()
            .is_none()
    );
    assert_eq!(web.inner().connected.load(Ordering::SeqCst), 1);
}

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

#[test]
fn whether_a_window_is_auxiliary_is_the_platform_answer() {
    let web = WebChannelAdapter::with_connector(
        PreparingPlatform::default(),
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    let mut window = agent_desktop_core::WindowInfo::for_process(&agent_desktop_core::AppInfo {
        name: "App".into(),
        pid: ProcessId::new(3),
        bundle_id: None,
        process_instance: None,
        presentation: None,
    });
    let deadline = Deadline::after(1_000).unwrap();
    window.id = "w-2".into();
    assert!(web.is_auxiliary_window(&window, deadline));
    window.id = "w-1".into();
    assert!(!web.is_auxiliary_window(&window, deadline));
}

#[test]
fn the_closed_menu_wait_grace_is_the_platform_answer() {
    let web = WebChannelAdapter::with_connector(
        PreparingPlatform::default(),
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    assert_ne!(
        agent_desktop_core::MENU_OPENER_GRACE,
        std::time::Duration::from_millis(300)
    );
    assert_eq!(
        web.menu_opener_grace(),
        std::time::Duration::from_millis(300)
    );
}
