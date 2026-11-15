//! Live legs for the `Sheet` arm of `surface_root`: each names a window and
//! none relies on the foreground, so a leg that passed only because its
//! fixture held the foreground would fail here. Each fixture leg holds
//! `FIXTURE_APP_NAME_LOCK` for the fixture's lifetime, per that lock's
//! contract.

use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, ObservationRoot, ProcessId, SnapshotSurface, WindowInfo,
};

use super::{surface_root, window_is_modal_sheet};
use crate::system::fixture::test_support::{FIXTURE_APP_NAME_LOCK, stage_foreground};
use crate::tree::element::UIAElement;
use crate::tree::fixture::fixture_modal::ModalFixture;
use crate::tree::fixture::{HostedFixture, bootstrap};

const STATE_TIMEOUT: Duration = Duration::from_secs(5);

fn sheet_of(handle: isize, pid: u32) -> Result<UIAElement, AdapterError> {
    let window = WindowInfo {
        id: format!("w-{handle}"),
        title: String::new(),
        app: String::new(),
        pid: ProcessId::from(pid),
        process_instance: None,
        bounds: None,
        state: Default::default(),
    };
    let deadline = Deadline::standard().expect("a standard deadline");
    surface_root(
        ObservationRoot::Window(&window),
        SnapshotSurface::Sheet,
        deadline,
    )
}

fn absence_of(result: Result<UIAElement, AdapterError>) -> AdapterError {
    match result {
        Ok(_) => panic!("expected the sheet surface to be absent, got a root"),
        Err(error) => error,
    }
}

fn open_modal() -> ModalFixture {
    let modal = ModalFixture::spawn().expect("the modal fixture spawns");
    modal.open();
    assert!(modal.wait_for_modal_state(true, STATE_TIMEOUT));
    modal
}

#[test]
fn a_window_without_a_sheet_reports_the_sheet_surface_absent() {
    bootstrap();
    let _scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let hosted = HostedFixture::spawn().expect("the hosted fixture spawns");

    let error = absence_of(sheet_of(hosted.handle(), hosted.process_id()));

    assert!(
        error.is_confirmed_surface_absence(SnapshotSurface::Sheet),
        "{error:?}"
    );
}

#[test]
fn the_owner_window_roots_the_modal_popup_it_owns() {
    bootstrap();
    let _scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let modal = open_modal();

    let root = sheet_of(modal.owner_handle(), modal.process_id()).expect("the owned modal roots");

    assert!(window_is_modal_sheet(&root));
}

#[test]
fn a_modal_window_roots_while_another_window_holds_the_foreground() {
    bootstrap();
    let _scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let modal = open_modal();
    let hosted = HostedFixture::spawn().expect("the hosted fixture spawns");
    if !stage_foreground(hosted.handle()) {
        eprintln!("skip: the OS declined the other window the foreground");
        return;
    }

    let root = sheet_of(modal.modal_handle(), modal.process_id())
        .expect("a background modal window roots as a sheet");

    assert!(window_is_modal_sheet(&root));
}

#[test]
fn a_closed_modal_leaves_its_owner_without_a_sheet() {
    bootstrap();
    let _scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let modal = open_modal();
    modal.close();
    assert!(modal.wait_for_modal_state(false, STATE_TIMEOUT));

    let error = absence_of(sheet_of(modal.owner_handle(), modal.process_id()));

    assert!(
        error.is_confirmed_surface_absence(SnapshotSurface::Sheet),
        "{error:?}"
    );
}
