use agent_desktop_core::{
    ActionOps, AdapterError, Deadline, InputOps, ObservationOps, PermissionReport, PermissionState,
    SystemOps,
};

use super::WebChannelAdapter;
use crate::channel::test_support::ScriptedConnector;

struct PreflightPlatform;

impl ObservationOps for PreflightPlatform {}
impl ActionOps for PreflightPlatform {}
impl InputOps for PreflightPlatform {}

impl SystemOps for PreflightPlatform {
    fn permission_report(&self, _deadline: Deadline) -> Result<PermissionReport, AdapterError> {
        Ok(PermissionReport {
            accessibility: PermissionState::Granted,
            screen_recording: PermissionState::Granted,
            automation: PermissionState::Granted,
        })
    }

    fn preflight_permission_report(
        &self,
        _deadline: Deadline,
    ) -> Result<PermissionReport, AdapterError> {
        Ok(PermissionReport {
            accessibility: PermissionState::Granted,
            screen_recording: PermissionState::Granted,
            automation: PermissionState::Unknown,
        })
    }
}

#[test]
fn a_command_preflight_reaches_the_platform_preflight_report() {
    let web = WebChannelAdapter::with_connector(
        PreflightPlatform,
        Box::new(ScriptedConnector::default()),
    )
    .with_download_ledger(None);
    let Ok(deadline) = Deadline::after(2_000) else {
        panic!("a two-second deadline is representable");
    };
    let Ok(report) = web.preflight_permission_report(deadline) else {
        panic!("the platform preflight report is infallible here");
    };
    assert_eq!(report.automation, PermissionState::Unknown);
}
