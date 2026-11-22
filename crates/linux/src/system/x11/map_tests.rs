use super::*;
use crate::system::x11::FrameExtents;

fn client(id: u32, pid: u32, title: &str, y: i32, top: u32) -> X11Client {
    X11Client {
        shadow: (0, 0, 0, 0),
        id,
        hidden: false,
        pid: Some(pid),
        title: title.to_owned(),
        extents: Some(FrameExtents {
            left: 0,
            right: 0,
            top,
            bottom: 0,
        }),
        bounds: Some(Extents {
            x: 100,
            y,
            width: 320,
            height: 200,
        }),
    }
}

fn frame(pid: u32, title: &str, extents: Option<Extents>, global: bool) -> FrameFacts {
    FrameFacts {
        size_identity: false,
        pid,
        title: title.to_owned(),
        extents,
        global_position: global,
    }
}

#[test]
fn extents_select_one_of_two_same_titled_windows_of_one_process() {
    let clients = [client(1, 10, "Doc", 157, 37), client(2, 10, "Doc", 400, 37)];
    let at = Some(Extents {
        x: 100,
        y: 120,
        width: 320,
        height: 237,
    });
    assert_eq!(
        match_client(&frame(10, "Doc", at, true), &clients),
        ClientMatch::One(1)
    );
}

fn openbox_client() -> X11Client {
    let mut client = client(1, 10, "Confirm File Format", 334, 20);
    client.extents = Some(FrameExtents {
        left: 1,
        right: 1,
        top: 20,
        bottom: 1,
    });
    client.bounds = Some(Extents {
        x: 343,
        y: 334,
        width: 595,
        height: 190,
    });
    client
}

fn openbox_frame(title: &str, global: bool) -> FrameFacts {
    let mut result = frame(
        10,
        title,
        Some(Extents {
            x: 342,
            y: 314,
            width: 597,
            height: 211,
        }),
        global,
    );
    result.size_identity = true;
    result
}

#[test]
fn openbox_borders_identify_a_geometry_fallback() {
    for global in [true, false] {
        assert_eq!(
            match_client(&openbox_frame("Question", global), &[openbox_client()]),
            ClientMatch::One(1)
        );
    }
}

#[test]
fn openbox_borders_break_a_title_tie() {
    let clients = [
        openbox_client(),
        client(2, 10, "Confirm File Format", 700, 20),
    ];
    assert_eq!(
        match_client(&openbox_frame("Confirm File Format", true), &clients),
        ClientMatch::One(1)
    );
}

#[test]
fn invalid_or_overflowing_frame_edges_identify_nothing() {
    let mut client = openbox_client();
    client.extents.as_mut().unwrap().left = u32::MAX;
    assert_eq!(frame_around(&client), None);
    client.extents.as_mut().unwrap().left = i32::MAX as u32;
    assert_eq!(frame_around(&client), None);
}

#[test]
fn extents_without_the_client_side_shadow_select_the_shadowed_client() {
    let mut first = client(1, 10, "Doc", 157, 0);
    first.shadow = (26, 23, 26, 29);
    let second = client(2, 10, "Doc", 400, 0);
    let visible = Some(Extents {
        x: 126,
        y: 180,
        width: 268,
        height: 148,
    });
    assert_eq!(
        match_client(&frame(10, "Doc", visible, true), &[first, second]),
        ClientMatch::One(1)
    );
}

#[test]
fn the_same_tie_for_window_relative_extents_stays_ambiguous() {
    let clients = [client(1, 10, "Doc", 157, 37), client(2, 10, "Doc", 400, 37)];
    let relative = Some(Extents {
        x: 0,
        y: 0,
        width: 320,
        height: 237,
    });
    assert_eq!(
        match_client(&frame(10, "Doc", relative, false), &clients),
        ClientMatch::Several(vec![1, 2])
    );
}

#[test]
fn a_tie_with_global_extents_that_match_neither_stays_ambiguous() {
    let clients = [client(1, 10, "Doc", 157, 37), client(2, 10, "Doc", 400, 37)];
    let nowhere = Some(Extents {
        x: 5,
        y: 5,
        width: 5,
        height: 5,
    });
    assert!(matches!(
        match_client(&frame(10, "Doc", nowhere, true), &clients),
        ClientMatch::Several(_)
    ));
}

fn ready(active: Option<u32>, clients: Vec<X11Client>) -> X11State {
    X11State::Ready { active, clients }
}

#[test]
fn the_active_client_names_the_listed_window_it_belongs_to() {
    let clients = vec![client(1, 10, "A", 50, 0), client(2, 11, "B", 50, 0)];
    let frames = [frame(10, "A", None, false), frame(11, "B", None, false)];
    assert_eq!(
        evidence(&ready(Some(2), clients), &frames),
        X11Evidence::Frame(1)
    );
}

#[test]
fn an_active_client_that_could_be_two_windows_is_ambiguous() {
    let clients = vec![client(1, 10, "Doc", 157, 37), client(2, 10, "Doc", 400, 37)];
    let relative = Some(Extents {
        x: 0,
        y: 0,
        width: 320,
        height: 237,
    });
    let frames = [
        frame(10, "Doc", relative, false),
        frame(10, "Doc", relative, false),
    ];
    assert_eq!(
        evidence(&ready(Some(1), clients), &frames),
        X11Evidence::Ambiguous
    );
}

#[test]
fn an_active_client_of_no_listed_window_is_unmapped() {
    let clients = vec![client(1, 10, "A", 50, 0)];
    assert_eq!(
        evidence(&ready(Some(1), clients), &[]),
        X11Evidence::Unmapped
    );
}

#[test]
fn no_active_window_and_no_window_manager_are_distinct_answers() {
    assert_eq!(evidence(&ready(None, vec![]), &[]), X11Evidence::NoActive);
    assert_eq!(
        evidence(&X11State::NoWindowManager, &[]),
        X11Evidence::Unavailable
    );
    assert_eq!(
        evidence(&X11State::Unavailable, &[]),
        X11Evidence::Unavailable
    );
}

#[test]
fn an_unnamed_local_frame_needs_one_exact_same_process_size_match() {
    let size = Some(Extents {
        x: 0,
        y: 0,
        width: 320,
        height: 200,
    });
    let unnamed = |size| {
        let mut panel = frame(10, "", size, false);
        panel.size_identity = true;
        panel
    };
    let panel = unnamed(size);
    let mut main = client(1, 10, "Calculator", 157, 0);
    main.bounds.as_mut().unwrap().width = 360;
    let shortcuts = client(2, 10, "gnome-calculator", 400, 0);
    let foreign = client(3, 11, "gnome-calculator", 400, 0);
    let clients = [main, shortcuts.clone(), foreign];
    assert_eq!(match_client(&panel, &clients), ClientMatch::One(2));
    assert_eq!(
        match_client(&panel, &[shortcuts, client(4, 10, "Another", 500, 0)]),
        ClientMatch::Several(vec![2, 4])
    );
    assert_eq!(
        match_client(&unnamed(None), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_client(&frame(10, "", size, false), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_client(&frame(10, "", size, false), &[client(7, 10, "", 500, 0)]),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_client(&frame(10, "Unrelated", size, false), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_client(
            &unnamed(Some(Extents {
                width: 0,
                ..Extents {
                    x: 0,
                    y: 0,
                    width: 320,
                    height: 200
                }
            })),
            &clients
        ),
        ClientMatch::NoClient
    );
}

#[test]
fn geometry_fallback_matches_native_title_differences_across_toolkits() {
    for (accessible, native, global, client_rect) in [
        ("Question", "", false, false),
        ("Question", "Confirm File Format", true, false),
        ("", "KCalc", true, true),
    ] {
        let client = client(1, 10, native, 157, 37);
        let rect = if client_rect {
            client.bounds
        } else {
            frame_around(&client)
        };
        let mut frame = frame(10, accessible, rect, global);
        frame.size_identity = true;
        assert_eq!(match_client(&frame, &[client]), ClientMatch::One(1));
    }
}

#[test]
fn geometry_fallback_preserves_ambiguity_and_requires_same_process_geometry() {
    let clients = [client(1, 10, "", 157, 37), client(2, 10, "Other", 400, 37)];
    let rect = frame_around(&clients[0]);
    let mut local = frame(10, "Question", rect, false);
    local.size_identity = true;
    assert_eq!(
        match_client(&local, &clients),
        ClientMatch::Several(vec![1, 2])
    );
    let global = frame(10, "Question", rect, true);
    assert_eq!(match_client(&global, &clients), ClientMatch::One(1));
    assert_eq!(
        match_client(&frame(11, "Question", rect, true), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_client(&frame(10, "Question", None, true), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(match_client(&global, &[]), ClientMatch::NoClient);
    assert_eq!(
        match_client(
            &frame(
                10,
                "Question",
                Some(Extents {
                    width: 0,
                    ..rect.unwrap()
                }),
                false
            ),
            &clients
        ),
        ClientMatch::NoClient
    );
}

#[test]
fn fallback_accepts_shadowless_frames_and_local_client_sizes() {
    let mut client = client(1, 10, "Native", 157, 37);
    client.shadow = (10, 10, 10, 10);
    let rect = without_shadow(&client, frame_around(&client).unwrap());
    for global in [false, true] {
        let mut frame = frame(10, "Accessible", Some(rect), global);
        frame.size_identity = true;
        assert_eq!(match_client(&frame, &[client.clone()]), ClientMatch::One(1));
    }
    let mut frame = frame(10, "Accessible", client.bounds, false);
    frame.size_identity = true;
    assert_eq!(match_client(&frame, &[client]), ClientMatch::One(1));
}

#[test]
fn a_title_match_keeps_precedence_over_a_geometry_match() {
    let clients = [
        client(1, 10, "Doc", 157, 37),
        client(2, 10, "Other", 400, 37),
    ];
    assert_eq!(
        match_client(&frame(10, "Doc", frame_around(&clients[1]), true), &clients),
        ClientMatch::One(1)
    );
}

#[test]
fn named_frames_reserve_their_clients_against_geometry_fallback() {
    let clients = vec![client(1, 10, "Doc", 157, 37)];
    let frames = [
        frame(10, "Doc", None, false),
        frame(10, "Question", frame_around(&clients[0]), true),
    ];
    assert_eq!(
        match_frames(&frames, &clients),
        vec![ClientMatch::One(1), ClientMatch::NoClient]
    );
    assert_eq!(
        evidence(&ready(Some(1), clients.clone()), &frames),
        X11Evidence::Frame(0)
    );
    let frames = [frames[1].clone(), frames[1].clone()];
    assert_eq!(
        match_frames(&frames, &clients),
        vec![ClientMatch::NoClient, ClientMatch::NoClient]
    );
}
