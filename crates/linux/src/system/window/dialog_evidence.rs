use agent_desktop_core::Deadline;
use x11rb::protocol::xproto::AtomEnum;

use crate::system::display::display_server::{DisplayServer, session_server};
use crate::system::focus::focus_world::focus_send;
use crate::system::x11::control::activate::first_window;
use crate::system::x11::control_display::open;
use crate::system::x11::stacking::bounded;

#[derive(Default)]
pub(super) struct DialogEvidence {
    pub(super) input_focus: bool,
    pub(super) transient_for: Option<u32>,
}

pub(super) fn read(client: u32, deadline: Deadline) -> DialogEvidence {
    let input_focus = focus_send::input_focus_in(client, deadline);
    let transient_for = if input_focus {
        bounded(deadline, None, move || {
            let DisplayServer::X11(display) = session_server() else {
                return None;
            };
            let (connection, _) = open(&display)?;
            first_window(&connection, client, AtomEnum::WM_TRANSIENT_FOR.into())
        })
    } else {
        None
    };
    DialogEvidence {
        input_focus,
        transient_for,
    }
}
