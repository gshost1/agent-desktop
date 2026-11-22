use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use agent_desktop_core::Deadline;
use x11rb::protocol::xproto::{Atom, AtomEnum};
use x11rb::rust_connection::RustConnection;

use super::DisplayReader;
use super::control_display::{atoms, longs, property};
use super::window::popup_window::PopupWindow;
use crate::system::display::display_server::{DisplayServer, session_server};

const MENU_SETTLE_TIME: Duration = Duration::from_millis(175);

struct MenuProbe {
    requests: Option<Sender<usize>>,
    answers: Receiver<bool>,
    worker: Option<super::bounded::PendingRead<()>>,
}

impl Drop for MenuProbe {
    fn drop(&mut self) {
        self.requests.take();
        self.worker.take();
    }
}

pub(crate) fn probe(pid: u32) -> impl FnMut(Deadline, usize) -> bool {
    let mut session = None;
    let mut unavailable = false;
    move |deadline, completed_read| {
        if unavailable || deadline.is_expired() {
            return false;
        }
        let session = session.get_or_insert_with(|| MenuProbe::new(session_server(), pid));
        let result = session.requests.as_ref().and_then(|sender| {
            sender.send(completed_read).ok()?;
            session.answers.recv_timeout(deadline.remaining()).ok()
        });
        if result.is_none() {
            unavailable = true;
            session.requests.take();
            session.worker.take();
        }
        result.unwrap_or(false)
    }
}

impl MenuProbe {
    fn new(server: DisplayServer, pid: u32) -> Self {
        Self::with_reader(move || {
            let DisplayServer::X11(display) = &server else {
                return None;
            };
            let (connection, screen) = DisplayReader::connect(display).ok()?;
            Some(move || super::reader::read_popups(&connection, screen, pid, true))
        })
    }

    fn with_reader<F>(connect: impl FnMut() -> Option<F> + Send + 'static) -> Self
    where
        F: FnMut() -> Option<Vec<PopupWindow>>,
    {
        Self::with_clock(connect, Instant::now)
    }

    fn with_clock<F>(
        mut connect: impl FnMut() -> Option<F> + Send + 'static,
        mut now: impl FnMut() -> Instant + Send + 'static,
    ) -> Self
    where
        F: FnMut() -> Option<Vec<PopupWindow>>,
    {
        let (requests, receive) = channel();
        let (send, answers) = channel();
        let worker = super::bounded::start(move || {
            let Some(mut read) = connect() else {
                return;
            };
            let mut generation = None;
            let mut mapped = HashMap::new();
            let mut accounted = Vec::new();
            while let Ok(completed_read) = receive.recv() {
                if generation != Some(completed_read) {
                    accounted = mapped.keys().copied().collect();
                    generation = Some(completed_read);
                }
                let Some(popups) = read().or_else(|| {
                    read = connect()?;
                    read()
                }) else {
                    return;
                };
                accounted.retain(|id| popups.iter().any(|popup| popup.id == *id));
                mapped.retain(|id, _| popups.iter().any(|popup| popup.id == *id));
                let observed = now();
                for popup in &popups {
                    mapped.entry(popup.id).or_insert(observed);
                }
                let settling = mapped
                    .values()
                    .any(|seen| observed.duration_since(*seen) < MENU_SETTLE_TIME);
                if send
                    .send(settling || unaccounted(&popups, &accounted))
                    .is_err()
                {
                    return;
                }
            }
        });
        Self {
            requests: Some(requests),
            answers,
            worker: Some(worker),
        }
    }
}

fn unaccounted(popups: &[PopupWindow], accounted: &[u32]) -> bool {
    popups.iter().any(|popup| !accounted.contains(&popup.id))
}

pub(super) fn menu_atoms(connection: &RustConnection) -> Option<Vec<Atom>> {
    atoms(
        connection,
        &[
            b"_NET_WM_WINDOW_TYPE",
            b"_NET_WM_WINDOW_TYPE_MENU",
            b"_NET_WM_WINDOW_TYPE_DROPDOWN_MENU",
            b"_NET_WM_WINDOW_TYPE_POPUP_MENU",
        ],
    )
}

fn has_menu_type(types: Option<&[Atom]>, menus: &[Atom]) -> bool {
    types.is_some_and(|types| types.iter().any(|kind| menus.contains(kind)))
}

pub(super) fn is_menu_window(connection: &RustConnection, window: u32, atoms: &[Atom]) -> bool {
    let types = property(connection, window, atoms[0], AtomEnum::ATOM, 16)
        .and_then(|reply| longs(reply.format, &reply.value, reply.bytes_after));
    has_menu_type(types.as_deref(), &atoms[1..])
}

#[cfg(test)]
#[path = "menu_pending_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "menu_pending_worker_tests.rs"]
mod worker_tests;
