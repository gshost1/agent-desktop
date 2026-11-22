use super::*;

#[test]
fn a_process_without_a_window_is_still_starting() {
    let environ = b"HOME=/home/user\0DISPLAY=:1\0".as_slice();
    assert_eq!(classify(Some(environ), || false), OffBus::Starting);
    assert_eq!(classify(None, || false), OffBus::Starting);
}

#[test]
fn a_window_without_bus_presence_is_windowed() {
    let environ = b"HOME=/home/user\0".as_slice();
    assert_eq!(classify(Some(environ), || true), OffBus::Windowed);
}

#[test]
fn an_environment_that_turns_the_bridge_off_needs_no_window_read() {
    for environ in [
        b"HOME=/home/user\0NO_AT_BRIDGE=1\0".as_slice(),
        b"GTK_A11Y=none\0DISPLAY=:1".as_slice(),
        b"GTK_A11Y=None\0".as_slice(),
    ] {
        let seen = classify(Some(environ), || panic!("the window read is not needed"));
        assert_eq!(seen, OffBus::BridgeOff);
    }
}

#[test]
fn a_bridge_setting_is_matched_as_a_whole_entry() {
    for environ in [
        b"NO_AT_BRIDGE=0\0".as_slice(),
        b"XNO_AT_BRIDGE=1\0".as_slice(),
        b"GTK_A11Y=atspi\0".as_slice(),
    ] {
        assert_eq!(classify(Some(environ), || false), OffBus::Starting);
    }
}

#[test]
fn a_window_owned_by_a_process_started_from_the_spawned_one_counts() {
    let me = std::process::id();
    assert!(descends_from(me, me));
    let parent = read_stat(me).unwrap().unwrap().parent;
    assert!(descends_from(me, parent));
    assert!(!descends_from(parent, me));
}

#[test]
fn a_native_title_is_readable_only_when_nonempty_and_valid() {
    assert_eq!(
        decode_title(b" Editor's window\0".to_vec()),
        Some("Editor's window".to_owned())
    );
    for bytes in [Vec::new(), b" \0".to_vec(), vec![255], b"one\0two".to_vec()] {
        assert_eq!(decode_title(bytes), None);
    }
}

#[cfg(test)]
fn live_window(
    connection: &RustConnection,
    root: Window,
    size: (u16, u16),
    properties: &[(Atom, Atom, u8, &[u8])],
) -> Window {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{CreateWindowAux, PropMode, WindowClass};

    let window = connection.generate_id().unwrap();
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            size.0,
            size.1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap()
        .check()
        .unwrap();
    for (property, kind, format, value) in properties {
        let units = u32::try_from(value.len()).unwrap() / u32::from(*format / 8);
        connection
            .change_property(
                PropMode::REPLACE,
                window,
                *property,
                *kind,
                *format,
                units,
                value,
            )
            .unwrap()
            .check()
            .unwrap();
    }
    window
}

#[test]
#[ignore = "needs a private X session; run through scripts/private-session.sh --wm"]
fn a_window_that_vanishes_or_is_malformed_mid_read_is_skipped() {
    let display = crate::system::x11::live_display::private_display();
    let (connection, root) = open(&display).unwrap();
    let [window_type, normal, name, utf8] = [
        b"_NET_WM_WINDOW_TYPE".as_slice(),
        b"_NET_WM_WINDOW_TYPE_NORMAL",
        b"_NET_WM_NAME",
        b"UTF8_STRING",
    ]
    .map(|atom| {
        connection
            .intern_atom(false, atom)
            .unwrap()
            .reply()
            .unwrap()
            .atom
    });
    let atoms = TitleAtoms::intern(&connection).unwrap();
    let main = live_window(
        &connection,
        root,
        (400, 300),
        &[
            (
                window_type,
                AtomEnum::ATOM.into(),
                32,
                &normal.to_ne_bytes(),
            ),
            (name, utf8, 8, b"Document"),
        ],
    );
    let malformed = live_window(
        &connection,
        root,
        (800, 600),
        &[
            (window_type, AtomEnum::STRING.into(), 8, b"normal"),
            (name, utf8, 8, b"Malformed"),
        ],
    );
    let vanished = live_window(&connection, root, (900, 700), &[]);
    connection
        .destroy_window(vanished)
        .unwrap()
        .check()
        .unwrap();
    let candidates = read_candidates(
        &connection,
        &atoms,
        [(0, main), (1, malformed), (2, vanished)],
    );
    assert_eq!(
        main_window::select_title(&candidates).as_deref(),
        Some("Document")
    );
    connection.destroy_window(main).unwrap().check().unwrap();
    connection
        .destroy_window(malformed)
        .unwrap()
        .check()
        .unwrap();
}

#[test]
#[ignore = "needs a private X session; run through scripts/private-session.sh --wm"]
fn reading_titles_creates_no_atom_on_the_server() {
    let display = crate::system::x11::live_display::private_display();
    let (connection, _) = open(&display).unwrap();
    let unknown = format!("AGENT_DESKTOP_UNSEEN_{}", std::process::id());
    let [utf8, missing] =
        intern_existing(&connection, [b"UTF8_STRING".as_slice(), unknown.as_bytes()]).unwrap();
    assert_ne!(utf8, x11rb::NONE);
    assert_eq!(missing, x11rb::NONE);
    let again = connection
        .intern_atom(true, unknown.as_bytes())
        .unwrap()
        .reply()
        .unwrap();
    assert_eq!(again.atom, x11rb::NONE);
}
