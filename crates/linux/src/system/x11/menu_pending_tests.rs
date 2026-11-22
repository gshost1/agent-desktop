use super::*;
use crate::system::display::{display_cause::Cause, display_unavailable::Unavailable};
use crate::tree::Extents;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;

fn read(display: &str, pid: u32) -> Option<bool> {
    let (connection, screen) = DisplayReader::connect(display).ok()?;
    Some(unaccounted(
        &super::super::reader::read_popups(&connection, screen, pid, true)?,
        &[],
    ))
}

#[test]
fn only_explicit_menu_types_establish_pending_evidence() {
    let menus = [7, 8, 9];
    for kind in menus {
        assert!(has_menu_type(Some(&[kind]), &menus));
        assert!(has_menu_type(Some(&[41, kind, 42]), &menus));
    }
    assert!(!has_menu_type(Some(&[41, 42]), &menus));
    assert!(!has_menu_type(Some(&[]), &menus));
    assert!(!has_menu_type(None, &menus));
}

fn popup(id: u32, x: i32) -> PopupWindow {
    PopupWindow {
        id,
        bounds: Extents {
            x,
            y: 20,
            width: 100,
            height: 100,
        },
    }
}

#[test]
fn only_unaccounted_popups_are_pending() {
    let first = popup(1, 10);
    let second = popup(2, 200);
    assert!(!unaccounted(&[], &[]));
    assert!(unaccounted(&[first], &[]));
    assert!(!unaccounted(&[first], &[first.id]));
    assert!(unaccounted(&[first, second], &[first.id]));
    assert!(!unaccounted(&[first, second], &[first.id, second.id]));
    assert!(unaccounted(&[first], &[second.id]));
}

fn poll(session: &MenuProbe, generation: usize) -> bool {
    session.requests.as_ref().unwrap().send(generation).unwrap();
    session
        .answers
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap()
}

fn advancing_reader<F>(connect: impl FnMut() -> Option<F> + Send + 'static) -> MenuProbe
where
    F: FnMut() -> Option<Vec<PopupWindow>>,
{
    let mut instant = Instant::now();
    MenuProbe::with_clock(connect, move || {
        instant += MENU_SETTLE_TIME;
        instant
    })
}

#[test]
fn newly_mapped_popup_requires_a_subsequent_complete_inventory() {
    let first = popup(1, 10);
    let second = popup(2, 10);
    let mut frames = std::collections::VecDeque::from([
        vec![],
        vec![first],
        vec![first],
        vec![first],
        vec![first, second],
        vec![first, second],
        vec![first, second],
        vec![second],
        vec![first, second],
        vec![first, second],
    ]);
    let mut frames = Some(std::mem::take(&mut frames));
    let session = advancing_reader(move || {
        let mut frames = frames.take()?;
        Some(move || frames.pop_front())
    });
    for (generation, expected) in [
        (1, false),
        (1, true),
        (1, true),
        (2, false),
        (2, true),
        (2, true),
        (3, false),
        (3, false),
        (3, true),
        (4, false),
    ] {
        assert_eq!(poll(&session, generation), expected, "read {generation}");
    }
}

#[test]
fn first_probe_cannot_account_for_stale_extents_from_the_initial_read() {
    let session = advancing_reader(|| Some(|| Some(vec![popup(1, 10)])));
    assert!(poll(&session, 1));
    assert!(poll(&session, 1));
    assert!(!poll(&session, 2));
}

#[test]
fn repeated_pending_probes_connect_once_and_read_fresh_windows() {
    let connections = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let connected = connections.clone();
    let scanned = reads.clone();
    let session = advancing_reader(move || {
        connected.fetch_add(1, Ordering::SeqCst);
        let scanned = scanned.clone();
        Some(move || {
            scanned.fetch_add(1, Ordering::SeqCst);
            Some(vec![popup(1, 10)])
        })
    });
    for (generation, expected) in [(1, true), (1, true), (2, false)] {
        assert_eq!(poll(&session, generation), expected);
    }
    drop(session);
    assert_eq!(connections.load(Ordering::SeqCst), 1);
    assert_eq!(reads.load(Ordering::SeqCst), 3);
}

#[test]
fn unreadable_and_non_x11_probes_close_without_pending_evidence() {
    for server in [
        DisplayServer::Wayland,
        DisplayServer::Unavailable(Unavailable {
            cause: Cause::Missing,
            detail: "no display".into(),
        }),
    ] {
        let session = MenuProbe::new(server, 1);
        session.requests.as_ref().unwrap().send(1).ok();
        assert!(
            session
                .answers
                .recv_timeout(std::time::Duration::from_secs(1))
                .is_err()
        );
    }
    let session = MenuProbe::with_reader(|| Some(|| None));
    session.requests.as_ref().unwrap().send(1).unwrap();
    assert!(
        session
            .answers
            .recv_timeout(std::time::Duration::from_secs(1))
            .is_err()
    );
}

#[test]
#[ignore = "needs a private X session; run through scripts/private-session.sh --wm"]
fn pending_live_requires_an_owned_mapped_override_redirect_menu() {
    use x11rb::protocol::xproto::{CreateWindowAux, PropMode, WindowClass};
    use x11rb::wrapper::ConnectionExt as _;

    let display = super::super::live_display::private_display();
    let (connection, screen) = DisplayReader::connect(&display).unwrap();
    let root = connection.setup().roots[screen].root;
    let window = connection.generate_id().unwrap();
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            71,
            83,
            280,
            190,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().override_redirect(1),
        )
        .unwrap()
        .check()
        .unwrap();
    let atoms = super::super::control_display::atoms(
        &connection,
        &[
            b"_NET_WM_PID",
            b"_NET_WM_WINDOW_TYPE",
            b"_NET_WM_WINDOW_TYPE_POPUP_MENU",
            b"_NET_WM_WINDOW_TYPE_TOOLTIP",
            b"_NET_WM_WINDOW_TYPE_COMBO",
        ],
    )
    .unwrap();
    let pid = std::process::id();
    connection
        .change_property32(
            PropMode::REPLACE,
            window,
            atoms[0],
            AtomEnum::CARDINAL,
            &[pid],
        )
        .unwrap()
        .check()
        .unwrap();
    connection
        .change_property32(
            PropMode::REPLACE,
            window,
            atoms[1],
            AtomEnum::ATOM,
            &[atoms[2]],
        )
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(read(&display, pid), Some(false));
    connection.map_window(window).unwrap().check().unwrap();
    connection.sync().unwrap();
    assert_eq!(read(&display, pid), Some(true));
    assert_eq!(read(&display, pid.wrapping_add(1)), Some(false));
    connection
        .delete_property(window, atoms[0])
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(read(&display, pid), Some(false));
    connection
        .change_property32(
            PropMode::REPLACE,
            window,
            atoms[0],
            AtomEnum::CARDINAL,
            &[pid],
        )
        .unwrap()
        .check()
        .unwrap();
    for kind in [atoms[3], atoms[4]] {
        connection
            .change_property32(PropMode::REPLACE, window, atoms[1], AtomEnum::ATOM, &[kind])
            .unwrap()
            .check()
            .unwrap();
        assert_eq!(read(&display, pid), Some(false));
    }
    connection
        .delete_property(window, atoms[1])
        .unwrap()
        .check()
        .unwrap();
    assert_eq!(read(&display, pid), Some(false));
    connection
        .change_property32(
            PropMode::REPLACE,
            window,
            atoms[1],
            AtomEnum::ATOM,
            &[atoms[2]],
        )
        .unwrap()
        .check()
        .unwrap();
    connection.unmap_window(window).unwrap().check().unwrap();
    assert_eq!(read(&display, pid), Some(false));
    connection
        .change_window_attributes(
            window,
            &x11rb::protocol::xproto::ChangeWindowAttributesAux::new().override_redirect(0),
        )
        .unwrap()
        .check()
        .unwrap();
    connection.map_window(window).unwrap().check().unwrap();
    assert_eq!(read(&display, pid), Some(false));
}
