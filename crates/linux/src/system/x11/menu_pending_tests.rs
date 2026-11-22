use super::*;

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

#[test]
fn unreadable_native_evidence_never_starts_a_wait() {
    assert!(!read_on(DisplayServer::X11(":7".into()), |_| None));
    assert!(!read_on(DisplayServer::X11(":7".into()), |_| Some(false)));
    assert!(read_on(DisplayServer::X11(":7".into()), |display| {
        assert_eq!(display, ":7");
        Some(true)
    }));
}

#[test]
fn wayland_never_reads_xwayland_for_pending_evidence() {
    assert!(!read_on(DisplayServer::Wayland, |_| panic!("not X11")));
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
    let atoms = atoms(
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
