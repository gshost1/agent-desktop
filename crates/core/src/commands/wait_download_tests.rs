use std::sync::Mutex;

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs, execute};
use crate::commands::wait_surface::SurfaceWait;
use crate::context::CommandContext;
use crate::{AdapterError, AppError, DownloadInfo, ErrorCode};

struct DownloadAdapter {
    outcome: fn() -> Result<DownloadInfo, AdapterError>,
    asked: Mutex<Vec<crate::ProcessIdentity>>,
}

impl ObservationOps for DownloadAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![crate::AppInfo {
            name: "Browser".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("browser-instance".into()),
            presentation: None,
        }])
    }
}

impl ActionOps for DownloadAdapter {}

impl InputOps for DownloadAdapter {}

impl SystemOps for DownloadAdapter {
    fn wait_for_download(
        &self,
        process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<DownloadInfo, AdapterError> {
        self.asked.lock().unwrap().push(process);
        (self.outcome)()
    }
}

fn saved() -> Result<DownloadInfo, AdapterError> {
    Ok(DownloadInfo {
        path: "/tmp/downloads/report.csv".into(),
        suggested_filename: "report.csv".into(),
        url: "https://example.test/report.csv".into(),
        state: "completed".into(),
        bytes: 8,
    })
}

fn timed_out() -> Result<DownloadInfo, AdapterError> {
    Err(AdapterError::timeout("still downloading")
        .with_details(serde_json::json!({ "kind": "cdp_download_in_progress", "guid": "g" })))
}

fn download_args(app: Option<&str>) -> WaitArgs {
    WaitArgs {
        mode: WaitModeArgs {
            surface: Some(SurfaceWait::Download),
            ..WaitModeArgs::default()
        },
        predicate: WaitPredicateArgs {
            predicate: None,
            value: None,
            action: None,
            count: None,
        },
        timeout_ms: 1_000,
        app: app.map(str::to_owned),
    }
}

fn run(
    outcome: fn() -> Result<DownloadInfo, AdapterError>,
    app: Option<&str>,
) -> (Result<serde_json::Value, AppError>, DownloadAdapter) {
    let adapter = DownloadAdapter {
        outcome,
        asked: Mutex::new(Vec::new()),
    };
    let result = execute(download_args(app), &adapter, &CommandContext::default());
    (result, adapter)
}

#[test]
fn a_download_wait_reports_the_saved_file_of_the_named_browser() {
    let (result, adapter) = run(saved, Some("Browser"));
    let value = result.unwrap();
    assert_eq!(value["path"], "/tmp/downloads/report.csv");
    assert_eq!(value["suggested_filename"], "report.csv");
    assert_eq!(value["state"], "completed");
    assert_eq!(value["bytes"], 8);
    assert!(value["elapsed_ms"].is_number());
    let asked = adapter.asked.lock().unwrap();
    assert_eq!(asked[0].pid, 42);
    assert_eq!(asked[0].instance, "browser-instance");
}

#[test]
fn a_download_wait_without_an_app_is_refused_before_the_adapter_is_asked() {
    let (result, adapter) = run(saved, None);
    assert_eq!(result.unwrap_err().code(), "INVALID_ARGS");
    assert!(adapter.asked.lock().unwrap().is_empty());
}

#[test]
fn a_download_wait_that_runs_out_keeps_the_adapter_cause() {
    let (result, _) = run(timed_out, Some("Browser"));
    let AppError::Adapter(error) = result.unwrap_err() else {
        panic!("expected an adapter error");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["predicate"], "download");
    assert_eq!(details["cause"], "cdp_download_in_progress");
    assert_eq!(details["guid"], "g");
}

#[test]
fn download_with_another_surface_flag_is_ambiguous() {
    assert!(SurfaceWait::with_download(Some(SurfaceWait::Menu), true).is_err());
    assert_eq!(
        SurfaceWait::with_download(None, true).unwrap(),
        Some(SurfaceWait::Download)
    );
    assert_eq!(
        SurfaceWait::with_download(Some(SurfaceWait::Notification), false).unwrap(),
        Some(SurfaceWait::Notification)
    );
}
