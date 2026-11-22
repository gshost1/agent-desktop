use super::reconcile_dialog as reconcile_with;
use crate::system::registry::listed_window::ListedWindow;
use crate::system::window::dialog_evidence::DialogEvidence;
use crate::system::window::window_test_support::{state as native_state, window as listed_window};
use crate::system::x11::X11State;
use crate::tree::Extents;

fn window(active: bool, modal: bool) -> ListedWindow {
    let mut window = listed_window(active, modal);
    window.frame.size_identity = true;
    window.frame.extents = Some(Extents {
        x: 0,
        y: 0,
        width: 500,
        height: 200,
    });
    window
}

fn state(pid: u32) -> X11State {
    let mut state = native_state(pid);
    if let X11State::Ready { clients, .. } = &mut state {
        clients[0].bounds = Some(Extents {
            x: 100,
            y: 100,
            width: 500,
            height: 200,
        });
        let mut parent = clients[0].clone();
        parent.id = 1;
        parent.title = "parent".into();
        parent.bounds = None;
        clients.push(parent);
    }
    state
}

fn reconcile_dialog(
    windows: &[ListedWindow],
    native: &X11State,
    input: impl FnOnce(u32) -> bool,
) -> Option<usize> {
    reconcile_with(windows, native, |id| DialogEvidence {
        input_focus: input(id),
        transient_for: Some(1),
    })
}

#[test]
fn stale_local_active_dialog_cannot_claim_an_unmatched_same_size_dialogs_client() {
    let native = state(7);
    let mut stale = window(true, true);
    stale.frame.global_position = false;
    stale.frame.size_identity = false;
    let mut actual = window(false, true);
    actual.frame.title = "different accessible name".into();
    actual.frame.global_position = false;
    actual.frame.size_identity = false;
    assert_eq!(
        crate::system::x11::map::match_frames(
            &[stale.frame.clone(), actual.frame.clone()],
            match &native {
                X11State::Ready { clients, .. } => clients,
                _ => unreachable!(),
            },
        ),
        vec![
            crate::system::x11::map::ClientMatch::NoClient,
            crate::system::x11::map::ClientMatch::NoClient,
        ]
    );
    assert_eq!(reconcile_dialog(&[stale, actual], &native, |_| true), None);
}

#[test]
fn transient_local_dialog_requires_size_identity_and_a_unique_listed_size() {
    let native = state(7);
    for proof in [false, true] {
        for competing in [false, true] {
            let mut dialog = window(true, true);
            dialog.frame.title = "parent".into();
            dialog.frame.global_position = false;
            dialog.frame.size_identity = proof;
            let mut windows = vec![dialog];
            if competing {
                let mut other = window(false, true);
                other.frame.global_position = false;
                other.frame.size_identity = false;
                windows.push(other);
            }
            assert_eq!(
                reconcile_dialog(&windows, &native, |_| true),
                if proof && !competing { Some(0) } else { None }
            );
        }
    }
}

#[test]
fn transient_global_dialog_without_size_identity_requires_exact_geometry() {
    for global in [false, true] {
        for x in [0, 100] {
            let mut dialog = window(true, true);
            dialog.frame.title = "parent".into();
            dialog.frame.size_identity = false;
            dialog.frame.global_position = global;
            dialog.frame.extents.as_mut().unwrap().x = x;
            dialog.frame.extents.as_mut().unwrap().y = 100;
            assert_eq!(
                reconcile_dialog(&[dialog], &state(7), |_| true),
                if global && x == 100 { Some(0) } else { None }
            );
        }
    }
}

#[test]
fn geometry_identifies_a_unique_dialog_but_not_competing_frames() {
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
            reconcile_with(&windows, &native, |_| DialogEvidence {
                input_focus: true,
                transient_for: None
            }),
            if competing { None } else { Some(0) }
        );
    }
}

#[test]
fn stale_active_dialog_cannot_claim_the_non_transient_main_window() {
    let dialog = window(true, true);
    let mut native = state(7);
    if let X11State::Ready { clients, .. } = &mut native {
        clients[0].title = "Main window".into();
        clients[0].bounds.as_mut().unwrap().width = 900;
    }
    assert_eq!(
        reconcile_with(&[dialog], &native, |_| DialogEvidence {
            input_focus: true,
            transient_for: None
        }),
        None
    );
}

#[test]
fn transient_dialog_requires_agreeing_known_size_and_a_distinct_same_pid_owner() {
    let native = state(7);
    for (width, owner, expected) in [
        (Some(500), Some(1), Some(0)),
        (Some(800), Some(1), None),
        (None, Some(1), None),
        (Some(500), Some(2), None),
        (Some(500), Some(99), None),
        (Some(500), None, None),
    ] {
        let mut dialog = window(true, true);
        dialog.frame.extents = width.map(|width| Extents {
            x: 0,
            y: 0,
            width,
            height: 200,
        });
        assert_eq!(
            reconcile_with(&[dialog], &native, |_| DialogEvidence {
                input_focus: true,
                transient_for: owner
            }),
            expected
        );
    }
    let mut foreign = state(7);
    if let X11State::Ready { clients, .. } = &mut foreign {
        clients[1].pid = Some(8);
    }
    assert_eq!(
        reconcile_dialog(&[window(true, true)], &foreign, |_| true),
        None
    );
}

#[test]
fn local_dialog_geometry_requires_size_identity_and_the_active_client() {
    let native = state(7);
    let mut dialog = window(true, true);
    dialog.frame.global_position = false;
    for proof in [false, true] {
        dialog.frame.size_identity = proof;
        assert_eq!(
            reconcile_with(&[dialog.clone()], &native, |_| DialogEvidence {
                input_focus: true,
                transient_for: None
            }),
            if proof { Some(0) } else { None }
        );
    }
    let mut wrong = state(7);
    if let X11State::Ready { active, clients } = &mut wrong {
        *active = Some(1);
        clients[1].bounds = Some(Extents {
            x: 100,
            y: 100,
            width: 900,
            height: 600,
        });
    }
    assert_eq!(
        reconcile_with(&[dialog], &wrong, |_| DialogEvidence {
            input_focus: true,
            transient_for: None
        }),
        None
    );
}

#[test]
fn local_geometry_reserves_the_title_matched_parents_client() {
    let mut native = state(7);
    if let X11State::Ready { clients, .. } = &mut native {
        clients[1].bounds = clients[0].bounds;
    }
    let mut parent = window(false, false);
    parent.frame.title = "parent".into();
    let mut dialog = window(true, true);
    dialog.frame.global_position = false;
    dialog.frame.size_identity = true;
    assert_eq!(
        reconcile_with(&[parent, dialog], &native, |_| DialogEvidence {
            input_focus: true,
            transient_for: None
        }),
        Some(1)
    );
}

#[test]
fn transient_size_can_match_client_frame_or_shadowless_frame() {
    let mut native = state(7);
    if let X11State::Ready { clients, .. } = &mut native {
        clients[0].extents = Some(crate::system::x11::FrameExtents {
            left: 5,
            right: 5,
            top: 20,
            bottom: 0,
        });
        clients[0].shadow = (2, 3, 2, 3);
    }
    for (width, height) in [(500, 200), (510, 220), (506, 214)] {
        let mut dialog = window(true, true);
        dialog.frame.extents = Some(Extents {
            x: 0,
            y: 0,
            width,
            height,
        });
        assert_eq!(reconcile_dialog(&[dialog], &native, |_| true), Some(0));
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
