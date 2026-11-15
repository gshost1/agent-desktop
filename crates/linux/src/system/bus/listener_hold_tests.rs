use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::spawn_holder;

fn eof_after(mut peer: UnixStream, limit: Duration) -> Option<Duration> {
    let started = Instant::now();
    peer.set_read_timeout(Some(limit)).unwrap();
    let mut byte = [0u8; 1];
    match peer.read(&mut byte) {
        Ok(0) => Some(started.elapsed()),
        _ => None,
    }
}

#[test]
fn the_holder_keeps_the_socket_open_after_the_caller_drops_it() {
    let (held, peer) = UnixStream::pair().unwrap();
    let mut child = spawn_holder(held.as_fd(), Duration::from_millis(400)).unwrap();
    drop(held);
    let closed = eof_after(peer, Duration::from_secs(5)).expect("the socket closes once held");
    assert!(
        closed >= Duration::from_millis(300),
        "the socket closed after {closed:?}, before the hold ended"
    );
    child.wait().unwrap();
}

#[test]
fn the_holder_inherits_no_other_descriptor_of_the_caller() {
    let (held, _peer) = UnixStream::pair().unwrap();
    let (stray, stray_peer) = UnixStream::pair().unwrap();
    assert_eq!(
        unsafe { libc::fcntl(stray.as_raw_fd(), libc::F_SETFD, 0) },
        0,
        "the stray descriptor is inheritable"
    );
    let mut child = spawn_holder(held.as_fd(), Duration::from_millis(800)).unwrap();
    drop(stray);
    let closed =
        eof_after(stray_peer, Duration::from_millis(400)).expect("no copy keeps the stray open");
    assert!(closed < Duration::from_millis(400));
    child.wait().unwrap();
}
