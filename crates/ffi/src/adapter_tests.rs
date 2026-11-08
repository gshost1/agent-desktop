use super::*;

fn registered_adapter(inner: Box<dyn PlatformAdapter>) -> *mut AdAdapter {
    register_adapter(AdAdapter {
        inner,
        session_id: None,
        _session_lease: None,
    })
    .unwrap()
}

#[test]
fn test_adapter_create_destroy() {
    let ptr = ad_adapter_create();
    assert!(!ptr.is_null());
    unsafe { ad_adapter_destroy(ptr) };
}

#[test]
fn test_destroy_null_is_noop() {
    unsafe { ad_adapter_destroy(std::ptr::null_mut()) };
}

#[test]
fn destroy_revokes_new_calls_without_invalidating_in_flight_owners() {
    let handle = ad_adapter_create();
    let retained = lookup_adapter(handle).unwrap();

    unsafe { ad_adapter_destroy(handle) };

    assert!(lookup_adapter(handle).is_err());
    let _ = retained
        .inner
        .permission_report(agent_desktop_core::Deadline::standard().unwrap());
}

struct UnknownPermissionAdapter;

impl ObservationOps for UnknownPermissionAdapter {}
impl ActionOps for UnknownPermissionAdapter {}
impl InputOps for UnknownPermissionAdapter {}

impl SystemOps for UnknownPermissionAdapter {
    fn permission_report(
        &self,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::PermissionReport, agent_desktop_core::AdapterError> {
        Ok(agent_desktop_core::PermissionReport {
            accessibility: PermissionState::Unknown,
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::NotRequired,
        })
    }
}

#[test]
fn check_permissions_maps_default_unknown_accessibility_to_platform_unsupported() {
    let adapter = registered_adapter(Box::new(UnknownPermissionAdapter));
    let result = unsafe { ad_check_permissions(adapter) };
    unsafe { ad_adapter_destroy(adapter) };

    assert_eq!(result, AdResult::ErrPlatformNotSupported);
}

struct AmbiguousPermissionAdapter;

impl ObservationOps for AmbiguousPermissionAdapter {}

impl ActionOps for AmbiguousPermissionAdapter {}

impl InputOps for AmbiguousPermissionAdapter {}

impl SystemOps for AmbiguousPermissionAdapter {
    fn permission_report(
        &self,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::PermissionReport, agent_desktop_core::AdapterError> {
        Ok(agent_desktop_core::PermissionReport {
            accessibility: PermissionState::Unknown,
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::NotRequired,
        })
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        false
    }
}

#[test]
fn check_permissions_preserves_ambiguous_unknown_accessibility_as_internal() {
    let adapter = registered_adapter(Box::new(AmbiguousPermissionAdapter));
    let result = unsafe { ad_check_permissions(adapter) };
    unsafe { ad_adapter_destroy(adapter) };

    assert_eq!(result, AdResult::ErrInternal);
}

static REGISTRY_FREE_DURING_TEARDOWN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct RegistryProbeAdapter;

impl ObservationOps for RegistryProbeAdapter {}
impl ActionOps for RegistryProbeAdapter {}
impl InputOps for RegistryProbeAdapter {}
impl SystemOps for RegistryProbeAdapter {}

impl Drop for RegistryProbeAdapter {
    fn drop(&mut self) {
        let free = (0..200).any(|_| {
            let acquired = adapters().try_lock().is_ok();
            if !acquired {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            acquired
        });
        REGISTRY_FREE_DURING_TEARDOWN.store(free, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn destroy_tears_down_the_adapter_outside_the_registry_lock() {
    let adapter = registered_adapter(Box::new(RegistryProbeAdapter));

    unsafe { ad_adapter_destroy(adapter) };

    assert!(REGISTRY_FREE_DURING_TEARDOWN.load(std::sync::atomic::Ordering::SeqCst));
}
