use agent_desktop_core::{WindowInfo, WindowState};

use crate::system::registry::listed_window::ListedWindow;
use crate::system::signal::signal_inventory::Pass;
use crate::system::x11::{FrameFacts, X11Client, X11State};

pub(super) fn window(active: bool, modal: bool) -> ListedWindow {
    ListedWindow {
        app: 0,
        info: WindowInfo {
            id: "w-dialog".into(),
            title: "accessible name".into(),
            app: "app".into(),
            pid: 7.into(),
            process_instance: Some("instance".into()),
            bounds: None,
            state: WindowState::default(),
        },
        active,
        modal,
        dialog: false,
        frame: FrameFacts {
            size_identity: false,
            pid: 7,
            title: "accessible name".into(),
            extents: None,
            global_position: true,
        },
        sheet: false,
        unshown: false,
    }
}

pub(super) fn state(pid: u32) -> X11State {
    X11State::Ready {
        active: Some(2),
        clients: vec![X11Client {
            id: 2,
            pid: Some(pid),
            title: "native name".into(),
            extents: None,
            bounds: None,
            hidden: false,
            shadow: (0, 0, 0, 0),
        }],
    }
}

pub(super) fn pass() -> Pass {
    Pass {
        windows: vec![window(true, true)],
        outcomes: Vec::new(),
        candidates: Vec::new(),
        focused: None,
        menu_focus_complete: true,
        recovered: Vec::new(),
    }
}
