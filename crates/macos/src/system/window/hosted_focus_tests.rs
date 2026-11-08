use super::*;

const HOST: i32 = 92_055;
const PANEL: i32 = 92_056;

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

fn panel_focus() -> FocusPids {
    FocusPids {
        host: HOST,
        host_focus: Some(PANEL),
        focused_application: Some(PANEL),
        ..FocusPids::default()
    }
}

fn save_as_field() -> Option<Rect> {
    Some(rect(620.0, 180.0, 230.0, 22.0))
}

#[test]
fn focus_in_a_panel_served_by_another_process_routes_keys_to_that_process() {
    let windows = document_and_sheet();
    let evidence = HostingEvidence {
        focus_frame: save_as_field(),
        host_windows: &windows,
        candidate_has_window: false,
    };

    assert_eq!(
        decide_route(&panel_focus(), &evidence),
        KeyboardRoute::HostedView { pid: PANEL }
    );
}

#[test]
fn window_server_presence_alone_proves_the_owner_can_receive_keys() {
    let windows = document_and_sheet();
    let pids = FocusPids {
        focused_application: Some(HOST),
        ..panel_focus()
    };
    let evidence = HostingEvidence {
        focus_frame: save_as_field(),
        host_windows: &windows,
        candidate_has_window: true,
    };

    assert_eq!(
        decide_route(&pids, &evidence),
        KeyboardRoute::HostedView { pid: PANEL }
    );
}

#[test]
fn host_owned_focus_keeps_keys_on_the_host() {
    let windows = document_and_sheet();
    let pids = FocusPids {
        host_focus: Some(HOST),
        system_focus: Some(HOST),
        ..panel_focus()
    };
    let evidence = HostingEvidence {
        focus_frame: save_as_field(),
        host_windows: &windows,
        candidate_has_window: true,
    };

    assert_eq!(decide_route(&pids, &evidence), KeyboardRoute::Host);
}

#[test]
fn another_applications_focus_never_redirects_keys() {
    let pids = FocusPids {
        host: HOST,
        host_focus: Some(HOST),
        system_focus: Some(PANEL),
        system_focus_is_host_focus: false,
        focused_application: Some(PANEL),
    };

    assert_eq!(focus_owner_candidate(&pids), None);
}

#[test]
fn system_focus_counts_when_it_is_the_hosts_own_focused_element() {
    let pids = FocusPids {
        host: HOST,
        host_focus: Some(HOST),
        system_focus: Some(PANEL),
        system_focus_is_host_focus: true,
        focused_application: Some(PANEL),
    };

    assert_eq!(focus_owner_candidate(&pids), Some(PANEL));
}

#[test]
fn focus_outside_every_host_window_is_unproven() {
    let windows = document_and_sheet();
    let evidence = HostingEvidence {
        focus_frame: Some(rect(1200.0, 800.0, 200.0, 22.0)),
        host_windows: &windows,
        candidate_has_window: true,
    };

    assert_eq!(
        decide_route(&panel_focus(), &evidence),
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "focus_outside_host_windows"
        }
    );
}

#[test]
fn an_owner_without_window_server_presence_is_unproven() {
    let windows = document_and_sheet();
    let pids = FocusPids {
        focused_application: Some(HOST),
        ..panel_focus()
    };
    let evidence = HostingEvidence {
        focus_frame: save_as_field(),
        host_windows: &windows,
        candidate_has_window: false,
    };

    assert_eq!(
        decide_route(&pids, &evidence),
        KeyboardRoute::Unproven {
            pid: PANEL,
            reason: "owner_has_no_window_server_presence"
        }
    );
}

#[test]
fn an_unknown_or_empty_focus_frame_is_unproven() {
    let windows = document_and_sheet();
    for focus_frame in [None, Some(rect(620.0, 180.0, 0.0, 22.0))] {
        let evidence = HostingEvidence {
            focus_frame,
            host_windows: &windows,
            candidate_has_window: true,
        };

        assert_eq!(
            decide_route(&panel_focus(), &evidence),
            KeyboardRoute::Unproven {
                pid: PANEL,
                reason: "focus_frame_unknown"
            }
        );
    }
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
fn route_details_name_the_pid_that_received_the_keys() {
    assert_eq!(route_details(HOST, KeyboardRoute::Host), None);
    assert_eq!(
        route_details(HOST, KeyboardRoute::HostedView { pid: PANEL }),
        Some(serde_json::json!({ "pid": PANEL, "host_pid": HOST, "route": "hosted_view" }))
    );
    assert_eq!(
        route_details(
            HOST,
            KeyboardRoute::Unproven {
                pid: PANEL,
                reason: "focus_outside_host_windows"
            }
        ),
        Some(serde_json::json!({
            "pid": HOST,
            "host_pid": HOST,
            "route": "host",
            "focus_owner_pid": PANEL,
            "unproven": "focus_outside_host_windows"
        }))
    );
}

#[test]
fn frames_match_within_one_point() {
    let field = rect(620.0, 180.0, 230.0, 22.0);

    assert!(same_frame(&field, &rect(620.5, 180.0, 230.0, 22.5)));
    assert!(!same_frame(&field, &rect(620.0, 184.0, 230.0, 22.0)));
}
