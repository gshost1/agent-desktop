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
