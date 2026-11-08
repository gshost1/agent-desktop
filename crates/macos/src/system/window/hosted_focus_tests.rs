use super::*;

const HOST: i32 = 11_741;
const PANEL: i32 = 11_829;
const OTHER: i32 = 4_242;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn document_and_sheet() -> Vec<Rect> {
    vec![
        rect(147.0, 72.0, 586.0, 488.0),
        rect(0.0, 140.0, 880.0, 448.0),
    ]
}

fn save_as_field() -> Option<Rect> {
    Some(rect(620.0, 180.0, 230.0, 22.0))
}

fn panel() -> Candidate {
    Candidate {
        pid: PANEL,
        ..Candidate::default()
    }
}

#[test]
fn a_focused_panel_service_working_for_the_app_receives_its_keys() {
    let candidates = [Candidate {
        focused_application: true,
        responsible_to_host: true,
        ..panel()
    }];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::HostedView { pid: PANEL }
    );
}

#[test]
fn a_windowed_process_showing_the_apps_focused_control_receives_its_keys() {
    let candidates = [Candidate {
        responsible_to_host: true,
        owns_window: true,
        focus_frame_matches: Some(true),
        ..panel()
    }];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::HostedView { pid: PANEL }
    );
}

#[test]
fn a_process_the_apps_focused_element_names_receives_keys_once_it_can_take_them() {
    let candidates = [Candidate {
        owns_host_focus: true,
        owns_window: true,
        ..panel()
    }];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::HostedView { pid: PANEL }
    );
}

#[test]
fn no_candidate_keeps_keys_on_the_app() {
    assert_eq!(decide_route(Some(true), &[]), KeyboardRoute::Host);
}

#[test]
fn an_unrelated_focused_application_is_not_linked_to_the_apps_control() {
    let candidates = [Candidate {
        pid: OTHER,
        focused_application: true,
        focus_frame_matches: Some(false),
        ..Candidate::default()
    }];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::Unproven {
            pid: OTHER,
            reason: "owner_focus_not_linked"
        }
    );
}

#[test]
fn an_owner_without_window_server_presence_is_unproven() {
    let candidates = [Candidate {
        owns_host_focus: true,
        responsible_to_host: true,
        ..panel()
    }];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "owner_has_no_window_server_presence"
        }
    );
}

#[test]
fn a_responsible_windowed_process_without_a_focus_link_is_unproven() {
    let candidates = [Candidate {
        responsible_to_host: true,
        owns_window: true,
        focus_frame_matches: Some(false),
        ..panel()
    }];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "owner_focus_not_linked"
        }
    );
}

#[test]
fn focus_outside_the_apps_windows_or_without_a_frame_is_unproven() {
    let candidates = [Candidate {
        focused_application: true,
        responsible_to_host: true,
        ..panel()
    }];

    assert_eq!(
        decide_route(Some(false), &candidates),
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "focus_outside_host_windows"
        }
    );
    assert_eq!(
        decide_route(None, &candidates),
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "focus_frame_unknown"
        }
    );
}

#[test]
fn the_first_proven_candidate_wins_over_an_unproven_one() {
    let candidates = [
        Candidate {
            pid: OTHER,
            responsible_to_host: true,
            owns_window: true,
            focus_frame_matches: Some(false),
            ..Candidate::default()
        },
        Candidate {
            focused_application: true,
            responsible_to_host: true,
            ..panel()
        },
    ];

    assert_eq!(
        decide_route(Some(true), &candidates),
        KeyboardRoute::HostedView { pid: PANEL }
    );
}

#[test]
fn only_processes_tied_to_the_apps_focus_are_relevant() {
    let windowed_child = Candidate {
        responsible_to_host: true,
        owns_window: true,
        focus_frame_matches: Some(false),
        ..panel()
    };

    assert!(!windowed_child.is_relevant(true));
    assert!(windowed_child.is_relevant(false));
    assert!(
        Candidate {
            focused_application: true,
            ..panel()
        }
        .is_relevant(true)
    );
}

#[test]
fn the_focus_frame_must_lie_inside_a_visible_app_window() {
    let windows = document_and_sheet();

    assert_eq!(frame_inside(save_as_field(), &windows), Some(true));
    assert_eq!(
        frame_inside(Some(rect(1200.0, 800.0, 200.0, 22.0)), &windows),
        Some(false)
    );
    assert_eq!(frame_inside(None, &windows), None);
    assert_eq!(
        frame_inside(Some(rect(620.0, 180.0, 0.0, 22.0)), &windows),
        None
    );
}

#[test]
fn frames_match_within_one_point() {
    let field = rect(620.0, 180.0, 230.0, 22.0);

    assert!(same_frame(&field, &rect(620.5, 180.0, 230.0, 22.5)));
    assert!(!same_frame(&field, &rect(620.0, 184.0, 230.0, 22.0)));
}

#[test]
fn only_a_hosted_view_route_changes_the_delivery_pid() {
    assert_eq!(
        KeyboardRoute::HostedView { pid: PANEL }.hosted_pid(),
        Some(PANEL)
    );
    assert_eq!(KeyboardRoute::Host.hosted_pid(), None);
    assert_eq!(
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "focus_frame_unknown"
        }
        .hosted_pid(),
        None
    );
}

#[test]
fn route_details_name_the_receiving_pid_and_every_candidates_evidence() {
    let candidates = [Candidate {
        focused_application: true,
        responsible_to_host: true,
        ..panel()
    }];
    let evidence = serde_json::json!([{
        "pid": PANEL,
        "owns_host_focus": false,
        "focused_application": true,
        "responsible_to_host": true,
        "focus_frame_matches": null,
        "owns_window": false,
    }]);

    assert_eq!(KeyboardRoute::Host.details(HOST, &candidates), None);
    assert_eq!(
        KeyboardRoute::HostedView { pid: PANEL }.details(HOST, &candidates),
        Some(serde_json::json!({
            "pid": PANEL, "host_pid": HOST, "route": "hosted_view", "candidates": evidence,
        }))
    );
    assert_eq!(
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "focus_outside_host_windows"
        }
        .details(HOST, &candidates),
        Some(serde_json::json!({
            "pid": HOST,
            "host_pid": HOST,
            "route": "host",
            "focus_owner_pid": PANEL,
            "unproven": "focus_outside_host_windows",
            "candidates": evidence,
        }))
    );
}
