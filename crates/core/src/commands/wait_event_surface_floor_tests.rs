use super::tests::{SequenceAdapter, empty_baseline, input};
use super::*;
use crate::SurfaceSignal;
use crate::snapshot::snapshot_surface::SnapshotSurface;

fn menu(id: &str) -> SurfaceSignal {
    SurfaceSignal {
        kind: SnapshotSurface::Menu,
        app: "TextEdit".into(),
        pid: crate::ProcessId::new(42),
        process_instance: "test-instance".into(),
        id: id.into(),
        title: None,
    }
}

fn with_surfaces(surfaces: Vec<SurfaceSignal>) -> SignalBaseline {
    SignalBaseline {
        surfaces,
        ..empty_baseline()
    }
}

#[test]
fn a_surface_that_closes_and_reopens_under_its_baseline_id_counts_as_appearing() {
    let adapter = SequenceAdapter::new(vec![
        with_surfaces(vec![menu("menu-7")]),
        with_surfaces(Vec::new()),
        with_surfaces(vec![menu("menu-7")]),
    ]);

    let result = wait_for_event(input("surface-appeared", None), &adapter, None).unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(result["event"]["kind"], "surface_appeared");
    assert_eq!(*adapter.calls.lock().unwrap(), 3);
}

#[test]
fn a_seeded_baseline_surface_that_reopens_counts_as_appearing() {
    let adapter = SequenceAdapter::new(vec![
        with_surfaces(Vec::new()),
        with_surfaces(vec![menu("menu-7")]),
    ]);

    let result = wait_for_event(
        input("surface-appeared", None),
        &adapter,
        Some(Ok(with_surfaces(vec![menu("menu-7")]))),
    )
    .unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(*adapter.calls.lock().unwrap(), 2);
}

#[test]
fn a_surface_present_throughout_the_wait_never_counts_as_appearing() {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let mut steady = input("surface-appeared", None);
    steady.timeout_ms = 200;
    let adapter = SequenceAdapter::new(vec![with_surfaces(vec![menu("menu-7")])]);

    let error = wait_for_event(steady, &adapter, None).unwrap_err();

    assert_eq!(error.code(), "TIMEOUT");
    assert!(*adapter.calls.lock().unwrap() > 2);
}

#[test]
fn a_surface_hidden_only_by_an_incomplete_capture_does_not_count_as_appearing() {
    let mut incomplete = with_surfaces(Vec::new());
    incomplete.completeness.surfaces = false;
    let adapter = SequenceAdapter::new(vec![
        with_surfaces(vec![menu("menu-7")]),
        incomplete,
        with_surfaces(vec![menu("menu-7")]),
        with_surfaces(vec![menu("menu-7"), menu("menu-8")]),
    ]);

    let result = wait_for_event(input("surface-appeared", None), &adapter, None).unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(*adapter.calls.lock().unwrap(), 4);
}

#[test]
fn a_surface_wait_that_starts_on_an_incomplete_read_still_sees_a_later_menu() {
    let mut incomplete = with_surfaces(Vec::new());
    incomplete.completeness.surfaces = false;
    let adapter = SequenceAdapter::new(vec![
        incomplete,
        with_surfaces(Vec::new()),
        with_surfaces(vec![menu("menu-7")]),
    ]);

    let result = wait_for_event(input("surface-appeared", None), &adapter, None).unwrap();

    assert_eq!(result["found"], true);
    assert_eq!(*adapter.calls.lock().unwrap(), 3);
}
