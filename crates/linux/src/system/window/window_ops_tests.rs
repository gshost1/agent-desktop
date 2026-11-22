use agent_desktop_core::{AdapterError, Deadline, ErrorCode, WindowInfo, WindowState};

use super::super::dialog_evidence::DialogEvidence;
use super::{finish_listing, finish_process_listing, require_answered, settle_pass};
use crate::system::registry::app_outcome::AppOutcome;
use crate::system::registry::listed_window::ListedWindow;
use crate::system::registry::registry_frames::RootRead;
use crate::system::signal::signal_inventory::pass_over_with_menu;
use crate::system::window::window_reread::reread_with;
use crate::system::window::window_test_support::{
    pass as inventory_pass, state as inventory_state,
};
use crate::system::x11::{FrameExtents, FrameFacts, X11Client, X11State};
use crate::tree::Extents;

#[test]
fn geometry_focus_does_not_also_run_dialog_reconciliation() {
    let bounds = Extents {
        x: 12,
        y: 34,
        width: 500,
        height: 200,
    };
    let mut dialog = window("Question", true, true);
    dialog.frame.extents = Some(bounds);
    let mut native = client(2, "native name");
    native.bounds = Some(bounds);
    let state = X11State::Ready {
        active: Some(2),
        clients: vec![native],
    };
    let mut pass = pass_over_with_menu(
        vec![dialog],
        Vec::new(),
        Vec::new(),
        &state,
        |_| Ok(false),
        |_| true,
    );
    assert_eq!(pass.focused, Some(0));
    assert_eq!(
        finish_listing(&mut pass, &state, None, |_| panic!("must not reconcile")),
        Some(0)
    );
    assert!(pass.windows[0].info.state.is_focused);
}

fn window(title: &str, active: bool, modal: bool) -> ListedWindow {
    ListedWindow {
        app: 0,
        info: WindowInfo {
            id: format!("w-1.1-{title}"),
            title: title.to_owned(),
            app: "a".to_owned(),
            pid: 7.into(),
            process_instance: Some("i".to_owned()),
            bounds: None,
            state: WindowState::default(),
        },
        active,
        modal,
        dialog: false,
        frame: FrameFacts {
            size_identity: false,
            pid: 7,
            title: title.to_owned(),
            extents: None,
            global_position: true,
        },
        sheet: false,
        unshown: false,
    }
}

fn client(id: u32, title: &str) -> X11Client {
    X11Client {
        shadow: (0, 0, 0, 0),
        id,
        pid: Some(7),
        title: title.to_owned(),
        extents: None::<FrameExtents>,
        bounds: None,
        hidden: false,
    }
}

#[test]
fn finish_reconciles_unmatched_dialog_and_marks_its_public_focus_flag() {
    let mut pass = crate::system::window::window_test_support::pass();
    let mut state = crate::system::window::window_test_support::state(7);
    let bounds = Extents {
        x: 10,
        y: 20,
        width: 500,
        height: 300,
    };
    pass.windows[0].frame.extents = Some(bounds);
    if let X11State::Ready { clients, .. } = &mut state {
        clients[0].bounds = Some(bounds);
    }
    assert_eq!(
        finish_listing(&mut pass, &state, None, |_| DialogEvidence {
            input_focus: true,
            transient_for: None
        }),
        Some(0)
    );
    assert!(pass.windows[0].info.state.is_focused);
}

#[test]
fn finish_preserves_an_existing_focus_decision() {
    let mut pass = crate::system::window::window_test_support::pass();
    pass.focused = Some(0);
    assert_eq!(
        finish_listing(&mut pass, &X11State::Unavailable, None, |_| panic!(
            "must not reconcile"
        )),
        Some(0)
    );
    assert!(pass.windows[0].info.state.is_focused);
}

#[test]
fn initial_and_reread_passes_refuse_stopped_targets_in_both_scoped_paths() {
    for named in [false, true] {
        let stopped = || Some(AdapterError::app_unresponsive("app"));
        let initial = settle_pass(inventory_pass(), stopped(), named)
            .err()
            .unwrap();
        assert_eq!(initial.code, ErrorCode::AppUnresponsive);
        let mut mapping_state = inventory_state(7);
        if let X11State::Ready { clients, .. } = &mut mapping_state {
            let mut main = clients[0].clone();
            main.id = 1;
            main.title = "accessible name".into();
            clients.push(main);
        }
        let error = reread_with(
            (
                settle_pass(inventory_pass(), None, named).unwrap(),
                mapping_state,
            ),
            Deadline::standard().unwrap(),
            &[7],
            |_| {
                settle_pass(inventory_pass(), stopped(), named)
                    .map(|pass| (pass, inventory_state(7)))
            },
            |_| {},
        )
        .err()
        .unwrap();
        assert_eq!(error.code, initial.code);
        assert_eq!(error.message, initial.message);
    }
}

#[test]
fn settlement_preserves_each_scoped_paths_unanswered_frame_contract() {
    for named in [false, true] {
        let mut unread = inventory_pass();
        unread.windows.clear();
        unread.outcomes = outcome(RootRead::TimedOut);
        let settled = settle_pass(unread, None, named);
        if named {
            assert_eq!(settled.err().unwrap().code, ErrorCode::Timeout);
        } else {
            let (windows, read) = finish_process_listing(settled.unwrap());
            assert!(windows.is_empty());
            assert_eq!(read, RootRead::TimedOut);
        }
    }
}

fn state(active: u32) -> X11State {
    X11State::Ready {
        active: Some(active),
        clients: vec![client(1, "main"), client(2, "dialog")],
    }
}

fn focused(
    listed: Vec<ListedWindow>,
    state: &X11State,
    input_focus_in: impl Fn(u32) -> bool,
) -> Option<usize> {
    pass_over_with_menu(
        listed,
        Vec::new(),
        Vec::new(),
        state,
        |_| Ok(false),
        input_focus_in,
    )
    .focused
}

#[test]
fn a_modal_frame_that_never_reports_active_is_focused_when_the_display_and_input_focus_name_it() {
    let listed = vec![window("main", false, false), window("dialog", false, true)];
    assert_eq!(focused(listed, &state(2), |_| true), Some(1));
}

#[test]
fn a_dialog_frame_the_display_does_not_name_is_not_focused() {
    let mut dialog = window("dialog", false, false);
    dialog.dialog = true;
    let listed = vec![window("main", false, false), dialog];
    assert_eq!(focused(listed, &state(1), |_| false), None);
}

#[test]
fn a_modal_frame_the_display_does_not_name_is_not_focused() {
    let listed = vec![window("main", false, false), window("dialog", false, true)];
    assert_eq!(focused(listed, &state(1), |_| false), None);
}

#[test]
fn a_frame_that_is_not_modal_and_not_active_is_never_focused_by_the_display_alone() {
    let listed = vec![window("main", false, false), window("dialog", false, false)];
    assert_eq!(focused(listed, &state(2), |_| true), None);
}

#[test]
fn an_inactive_dialog_needs_positive_input_focus_evidence() {
    for modal in [false, true] {
        let mut dialog = window("dialog", false, modal);
        dialog.dialog = true;
        let listed = vec![window("main", false, false), dialog];
        assert_eq!(focused(listed.clone(), &state(2), |_| false), None);
        assert_eq!(focused(listed.clone(), &state(2), |id| id == 2), Some(1));
        assert_eq!(focused(listed, &state(2), |id| id == 1), None);
    }
}

fn outcome(read: RootRead) -> Vec<AppOutcome> {
    vec![AppOutcome { app: 0, read }]
}

#[test]
fn a_named_application_that_left_a_frame_unanswered_and_listed_nothing_is_a_timeout() {
    let error = require_answered(Vec::new(), &outcome(RootRead::TimedOut), true)
        .expect_err("an empty list must not read as a closed window");
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn an_empty_list_stays_empty_when_every_frame_answered_or_no_application_was_named() {
    assert!(
        require_answered(Vec::new(), &outcome(RootRead::Read), true)
            .expect("an answered read stays empty")
            .is_empty()
    );
    assert!(
        require_answered(Vec::new(), &outcome(RootRead::TimedOut), false)
            .expect("an unnamed listing stays empty")
            .is_empty()
    );
}

#[test]
fn the_windows_that_did_answer_are_listed_beside_an_unanswered_frame() {
    let listed = vec![window("main", false, false)];
    let kept = require_answered(listed, &outcome(RootRead::TimedOut), true)
        .expect("the answered windows are kept");
    assert_eq!(kept.len(), 1);
}

#[test]
fn process_inventory_keeps_windows_and_reports_any_unanswered_frames() {
    for reads in [
        vec![RootRead::Read, RootRead::Read],
        vec![RootRead::Read, RootRead::TimedOut],
        vec![RootRead::TimedOut, RootRead::Read],
    ] {
        for listed in [Vec::new(), vec![window("main", false, false)]] {
            let expected_count = listed.len();
            let outcomes = reads
                .iter()
                .enumerate()
                .map(|(app, read)| AppOutcome { app, read: *read })
                .collect();
            let pass = pass_over_with_menu(
                listed,
                outcomes,
                Vec::new(),
                &X11State::Unavailable,
                |_| Ok(false),
                |_| false,
            );
            let (windows, read) = finish_process_listing(pass);
            assert_eq!(windows.len(), expected_count);
            assert_eq!(
                read == RootRead::TimedOut,
                reads.contains(&RootRead::TimedOut)
            );
        }
    }
}
