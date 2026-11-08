use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::Mutex;

struct RecordingAdapter {
    granted: Mutex<Option<Duration>>,
    result: fn() -> Result<Vec<SurfaceInfo>, AdapterError>,
}

impl ObservationOps for RecordingAdapter {
    fn list_surfaces(
        &self,
        _process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<Vec<SurfaceInfo>, AdapterError> {
        *self.granted.lock().expect("lock") = Some(deadline.remaining());
        (self.result)()
    }
}
impl ActionOps for RecordingAdapter {}
impl InputOps for RecordingAdapter {}
impl SystemOps for RecordingAdapter {}

fn sheet() -> Result<Vec<SurfaceInfo>, AdapterError> {
    Ok(vec![SurfaceInfo {
        id: "focused-window".into(),
        kind: "sheet".into(),
        title: Some("Save".into()),
        item_count: None,
        unclassified: Vec::new(),
    }])
}

fn process() -> ProcessIdentity {
    ProcessIdentity::new(42, "test-instance")
}

#[test]
fn a_long_action_budget_does_not_extend_the_surface_report() {
    let adapter = RecordingAdapter {
        granted: Mutex::new(None),
        result: sheet,
    };
    let action_deadline = Deadline::after(30_000).expect("deadline");

    let surfaces = read(&adapter, process(), action_deadline);

    assert_eq!(surfaces.len(), 1);
    let granted = adapter
        .granted
        .lock()
        .expect("lock")
        .expect("surfaces were read");
    assert!(
        granted <= SETTLED_SURFACE_BUDGET,
        "surface report got {granted:?}"
    );
}

#[test]
fn a_short_action_budget_still_bounds_the_surface_report() {
    let adapter = RecordingAdapter {
        granted: Mutex::new(None),
        result: sheet,
    };

    read(&adapter, process(), Deadline::after(200).expect("deadline"));

    let granted = adapter
        .granted
        .lock()
        .expect("lock")
        .expect("surfaces were read");
    assert!(granted <= Duration::from_millis(200));
}

#[test]
fn a_timed_out_surface_report_leaves_the_result_without_surfaces() {
    let adapter = RecordingAdapter {
        granted: Mutex::new(None),
        result: || Err(AdapterError::timeout("surface walk exceeded its deadline")),
    };

    let surfaces = read(
        &adapter,
        process(),
        Deadline::after(5_000).expect("deadline"),
    );

    assert!(surfaces.is_empty());
}

fn sheet_and_web_page() -> Result<Vec<SurfaceInfo>, AdapterError> {
    let mut surfaces = sheet()?;
    surfaces.push(SurfaceInfo {
        id: "web:page-1".into(),
        kind: crate::SnapshotSurface::Web.as_str().into(),
        title: Some("Example".into()),
        item_count: None,
        unclassified: Vec::new(),
    });
    Ok(surfaces)
}

#[test]
fn an_open_web_page_is_not_reported_as_a_surface_the_action_raised() {
    let adapter = RecordingAdapter {
        granted: Mutex::new(None),
        result: sheet_and_web_page,
    };

    let surfaces = read(
        &adapter,
        process(),
        Deadline::after(5_000).expect("deadline"),
    );

    let kinds: Vec<&str> = surfaces
        .iter()
        .map(|surface| surface.kind.as_str())
        .collect();
    assert_eq!(kinds, vec!["sheet"]);
}
