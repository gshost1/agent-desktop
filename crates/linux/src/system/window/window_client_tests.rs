use super::*;
use crate::system::x11::map::{match_client, match_frames};

struct World {
    global: bool,
    frames: Option<(Vec<FrameFacts>, usize)>,
}

impl ClientWorld for World {
    fn global_position(&self) -> bool {
        self.global
    }

    fn frames(&self, _win: &WindowInfo) -> Option<(Vec<FrameFacts>, usize)> {
        self.frames.clone()
    }
}

fn rectangle(width: i32) -> Extents {
    Extents {
        x: 10,
        y: 20,
        width,
        height: 300,
    }
}

fn window() -> WindowInfo {
    let mut win = crate::system::window::window_test_support::window(false, false).info;
    win.title = "Question".into();
    win.bounds = Some(rectangle(400).into());
    win
}

fn client(title: &str, width: i32) -> X11Client {
    X11Client {
        id: 1,
        pid: Some(7),
        title: title.into(),
        bounds: Some(rectangle(width)),
        extents: None,
        shadow: (0, 0, 0, 0),
        hidden: false,
    }
}

#[test]
fn ghost_dialog_never_reclaims_the_title_matched_main_client() {
    let win = window();
    let clients = [client("Doc", 400)];
    for global in [false, true] {
        let ghost = facts_of(&win, global);
        let mut main = ghost.clone();
        main.title = "Doc".into();
        let frames = vec![main, ghost];
        assert_eq!(
            match_frames(&frames, &clients),
            vec![ClientMatch::One(1), ClientMatch::NoClient]
        );
        for frames in [Some((frames.clone(), 1)), None] {
            let world = World { global, frames };
            assert_eq!(
                client_of(&world, &win, &clients, || Some(1)),
                Err(Cause::WindowNotMapped)
            );
        }
    }
}

#[test]
fn titled_local_geometry_requires_size_identity() {
    let win = window();
    let clients = [client("Native", 400)];
    let mut frame = facts_of(&win, false);
    assert_eq!(match_client(&frame, &clients), ClientMatch::NoClient);
    frame.size_identity = true;
    assert_eq!(match_client(&frame, &clients), ClientMatch::One(1));
}

#[test]
fn unique_sized_dialog_and_unnamed_global_frame_keep_geometry_matching() {
    for global in [false, true] {
        let mut win = window();
        if global {
            win.title.clear();
        }
        let mut target = facts_of(&win, global);
        target.size_identity = !global;
        let mut main = target.clone();
        main.title = "Doc".into();
        main.extents = Some(rectangle(800));
        let mut native = client(if global { "KCalc" } else { "Native" }, 400);
        native.id = 2;
        let clients = [client("Doc", 800), native];
        let world = World {
            global,
            frames: Some((vec![main, target], 1)),
        };
        assert_eq!(client_of(&world, &win, &clients, || Some(2)), Ok(2));
    }
}

#[test]
fn missing_or_invalid_frame_inventory_never_supplies_geometry_identity() {
    let win = window();
    for frames in [None, Some((Vec::new(), 0))] {
        let world = World {
            global: true,
            frames,
        };
        assert_eq!(
            client_of(&world, &win, &[client("Doc", 400)], || None),
            Err(Cause::WindowNotMapped)
        );
    }
}
