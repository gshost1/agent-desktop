use std::sync::atomic::{AtomicUsize, Ordering};

use super::resolve_window_for_surface;
use crate::{
    AdapterError, AppError, AppInfo, Deadline, ErrorCode, ProcessIdentity, SnapshotSurface,
    WindowInfo, adapter::WindowFilter,
};

const RECORDED_INSTANCE: &str = "inst-7";

struct BrowserProcess {
    windows: Vec<WindowInfo>,
    instance: &'static str,
    recorded_instance: Option<&'static str>,
    endpoint_reads: AtomicUsize,
}

impl BrowserProcess {
    fn windowless(recorded_instance: Option<&'static str>) -> Self {
        Self {
            windows: Vec::new(),
            instance: RECORDED_INSTANCE,
            recorded_instance,
            endpoint_reads: AtomicUsize::new(0),
        }
    }
}

impl crate::ObservationOps for BrowserProcess {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.windows.clone())
    }

    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "Browser".into(),
            pid: crate::ProcessId::new(7),
            process_instance: Some(self.instance.into()),
            bundle_id: None,
            presentation: Some(crate::AppPresentation::Foreground),
        }])
    }
}
impl crate::ActionOps for BrowserProcess {}
impl crate::InputOps for BrowserProcess {}
impl crate::SystemOps for BrowserProcess {
    fn has_recorded_cdp_endpoint(&self, process: ProcessIdentity) -> bool {
        self.endpoint_reads.fetch_add(1, Ordering::SeqCst);
        self.recorded_instance
            .is_some_and(|instance| process == ProcessIdentity::new(7u32, instance))
    }
}

fn resolve(adapter: &BrowserProcess, surface: SnapshotSurface) -> Result<WindowInfo, AppError> {
    resolve_window_for_surface(
        adapter,
        Some("Browser"),
        None,
        surface,
        Deadline::after(2_000).unwrap(),
    )
}

fn assert_no_owner_window(result: Result<WindowInfo, AppError>, surface: &str) {
    let Err(AppError::Adapter(error)) = result else {
        panic!("expected today's window error, got {result:?}");
    };
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(
        error.message,
        format!("No window found to identify the application owning surface '{surface}'")
    );
}

#[test]
fn a_windowless_browser_with_a_recorded_endpoint_owns_the_web_surface_through_its_process() {
    let browser = BrowserProcess::windowless(Some(RECORDED_INSTANCE));
    let owner = resolve(&browser, SnapshotSurface::Web).unwrap();
    assert!(owner.is_process_owner());
    assert_eq!(owner.app, "Browser");
    assert_eq!(owner.pid, crate::ProcessId::new(7));
    assert_eq!(owner.process_instance.as_deref(), Some(RECORDED_INSTANCE));
}

#[test]
fn a_windowless_browser_without_a_recorded_endpoint_keeps_the_window_error() {
    let browser = BrowserProcess::windowless(None);
    assert_no_owner_window(resolve(&browser, SnapshotSurface::Web), "web");
}

#[test]
fn an_endpoint_recorded_for_an_earlier_instance_does_not_own_the_web_surface() {
    let browser = BrowserProcess {
        instance: "inst-8",
        ..BrowserProcess::windowless(Some(RECORDED_INSTANCE))
    };
    assert_no_owner_window(resolve(&browser, SnapshotSurface::Web), "web");
}

#[test]
fn a_recorded_endpoint_does_not_stand_in_for_a_window_on_other_surfaces() {
    let browser = BrowserProcess::windowless(Some(RECORDED_INSTANCE));
    assert_no_owner_window(resolve(&browser, SnapshotSurface::Popover), "popover");
    assert_eq!(browser.endpoint_reads.load(Ordering::SeqCst), 0);
}

#[test]
fn a_listed_window_owns_the_web_surface_without_reading_the_endpoint_record() {
    let window = WindowInfo {
        id: "w1".into(),
        title: "Docs".into(),
        app: "Browser".into(),
        pid: crate::ProcessId::new(7),
        process_instance: Some(RECORDED_INSTANCE.into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            visible: Some(true),
            accessible: true,
            ..Default::default()
        },
    };
    let browser = BrowserProcess {
        windows: vec![window],
        ..BrowserProcess::windowless(Some(RECORDED_INSTANCE))
    };
    let owner = resolve(&browser, SnapshotSurface::Web).unwrap();
    assert_eq!(owner.id, "w1");
    assert_eq!(browser.endpoint_reads.load(Ordering::SeqCst), 0);
}
