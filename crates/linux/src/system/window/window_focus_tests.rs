use super::reconcile_dialog;
use crate::system::window::window_test_support::{state, window};
use crate::system::x11::X11State;
use crate::tree::Extents;

#[test]
fn geometry_matches_including_competing_frames_cannot_be_reconciled() {
    let bounds = Extents {
        x: 12,
        y: 34,
        width: 500,
        height: 200,
    };
    for competing in [false, true] {
        let mut native = state(7);
        if let X11State::Ready { clients, .. } = &mut native {
            clients[0].bounds = Some(bounds);
        }
        let mut dialog = window(true, true);
        dialog.frame.extents = Some(bounds);
        let mut windows = vec![dialog.clone()];
        if competing {
            dialog.active = false;
            windows.push(dialog);
        }
        assert_eq!(
            reconcile_dialog(&windows, &native, |_| panic!("already matches")),
            None
        );
    }
}

#[test]
fn unmatched_active_modal_or_dialog_needs_native_pid_and_input_focus() {
    for modal in [true, false] {
        let mut dialog = window(true, modal);
        dialog.dialog = !modal;
        assert_eq!(
            reconcile_dialog(&[dialog], &state(7), |id| id == 2),
            Some(0)
        );
    }
}

#[test]
fn foreign_native_pid_is_not_reconciled() {
    assert_eq!(
        reconcile_dialog(&[window(true, true)], &state(8), |_| true),
        None
    );
}

#[test]
fn two_active_frames_of_the_native_pid_are_not_reconciled() {
    for modal in [true, false] {
        assert_eq!(
            reconcile_dialog(
                &[window(true, true), window(true, modal)],
                &state(7),
                |_| true
            ),
            None
        );
    }
}

#[test]
fn another_pids_active_frame_does_not_compete() {
    let mut foreign = window(true, true);
    foreign.info.pid = 8.into();
    foreign.frame.pid = 8;
    assert_eq!(
        reconcile_dialog(&[foreign, window(true, true)], &state(7), |_| true),
        Some(1)
    );
}

#[test]
fn inactive_or_non_dialog_frame_is_not_reconciled() {
    for candidate in [window(false, true), window(true, false)] {
        assert_eq!(reconcile_dialog(&[candidate], &state(7), |_| true), None);
    }
}

#[test]
fn mapped_or_ambiguously_mapped_active_client_is_not_reconciled() {
    let mut matched = window(false, false);
    matched.frame.title = "native name".into();
    for others in [vec![matched.clone()], vec![matched.clone(), matched]] {
        let mut windows = vec![window(true, true)];
        windows.extend(others);
        assert_eq!(reconcile_dialog(&windows, &state(7), |_| true), None);
    }
}

#[test]
fn input_focus_outside_the_client_is_not_reconciled() {
    assert_eq!(
        reconcile_dialog(&[window(true, true)], &state(7), |_| false),
        None
    );
}

#[test]
fn missing_native_identity_or_windows_cannot_reconcile() {
    let mut no_pid = state(7);
    if let X11State::Ready { clients, .. } = &mut no_pid {
        clients[0].pid = None;
    }
    for native in [
        X11State::Unavailable,
        X11State::NoWindowManager,
        no_pid,
        X11State::Ready {
            active: None,
            clients: Vec::new(),
        },
        X11State::Ready {
            active: Some(2),
            clients: Vec::new(),
        },
    ] {
        assert_eq!(
            reconcile_dialog(&[window(true, true)], &native, |_| true),
            None
        );
    }
    assert_eq!(reconcile_dialog(&[], &state(7), |_| true), None);
}
