use super::{is_daemon_launch, launch_socket};
use std::ffi::OsStr;
use std::os::fd::AsFd;
use std::os::unix::net::UnixDatagram;

#[test]
fn startup_requires_explicit_opt_in_even_with_the_parent_socket() {
    let socket = launch_socket().unwrap();
    assert!(!is_daemon_launch(None, socket.as_fd()));
    assert!(!is_daemon_launch(Some(OsStr::new("0")), socket.as_fd()));
    assert!(is_daemon_launch(Some(OsStr::new("1")), socket.as_fd()));
}

#[test]
fn startup_requires_a_parent_frame_even_with_opt_in() {
    let (sender, receiver) = UnixDatagram::pair().unwrap();
    let enabled = Some(OsStr::new("1"));
    assert!(!is_daemon_launch(enabled, receiver.as_fd()));
    sender.send(b"ordinary stdin").unwrap();
    assert!(!is_daemon_launch(enabled, receiver.as_fd()));
    let mut data = [0; 32];
    let count = receiver.recv(&mut data).unwrap();
    assert_eq!(&data[..count], b"ordinary stdin");
}
