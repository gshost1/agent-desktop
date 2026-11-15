use std::cell::Cell;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

use std::sync::Mutex;

use super::{ListenerHold, builder_with, hold_after};

fn alive(pid: libc::pid_t) -> bool {
    unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) == 0 }
}

fn finish(pid: libc::pid_t) {
    unsafe {
        libc::kill(pid, libc::SIGKILL);
        libc::waitpid(pid, std::ptr::null_mut(), 0);
    }
}

#[test]
fn a_key_send_is_held_even_when_it_failed_and_a_pointer_send_is_not() {
    let holds = Cell::new(0);
    let failed: Result<(), &str> =
        hold_after(true, || Err("stopped"), || holds.set(holds.get() + 1));
    assert!(failed.is_err());
    assert_eq!(holds.get(), 1);
    let pointer: Result<(), &str> = hold_after(false, || Ok(()), || holds.set(holds.get() + 1));
    assert!(pointer.is_ok());
    assert_eq!(holds.get(), 1);
}

#[test]
fn two_holds_in_a_row_leave_exactly_one_holder() {
    let (held, _peer) = UnixStream::pair().unwrap();
    let mut hold = ListenerHold::default();
    hold.remember(&held, Duration::from_secs(5));
    let first = hold
        .after_keys(Duration::from_secs(5))
        .expect("first holder");
    let started = std::time::Instant::now();
    let second = hold
        .after_keys(Duration::from_secs(5))
        .expect("second holder");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "replacing the holder waited {:?} for the first one to end",
        started.elapsed()
    );
    let mut status = 0;
    let reaped = unsafe { libc::waitpid(first, &mut status, libc::WNOHANG) };
    assert_eq!(reaped, -1, "the first holder was already killed and reaped");
    assert!(alive(second), "the second holder holds the socket");
    finish(second);
}

#[test]
fn a_listener_registered_right_after_keys_is_held_at_once() {
    let (held, _peer) = UnixStream::pair().unwrap();
    let mut hold = ListenerHold::default();
    assert_eq!(
        hold.after_keys(Duration::from_secs(5)),
        None,
        "nothing to hold yet"
    );
    hold.remember(&held, Duration::from_secs(5));
    let child = hold.child.as_ref().expect("the new listener is held").pid();
    assert!(alive(child));
    finish(child);
}

#[test]
fn the_remembered_socket_sits_above_the_standard_descriptors() {
    let (held, _peer) = UnixStream::pair().unwrap();
    let mut hold = ListenerHold::default();
    hold.remember(&held, Duration::from_secs(5));
    let copy = std::os::fd::AsRawFd::as_raw_fd(hold.socket.as_ref().expect("remembered"));
    assert!(copy >= super::SOCKET_FLOOR);
}

#[test]
fn a_unix_address_is_remembered_and_any_other_forgets_it() {
    let dir = std::env::temp_dir().join(format!("ad-listener-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bus");
    let _listener = UnixListener::bind(&path).unwrap();
    let hold = Mutex::new(ListenerHold::default());
    let address = format!(
        "unix:path={},guid=0123456789abcdef0123456789abcdef",
        path.display()
    );
    let _connected =
        futures_lite::future::block_on(builder_with(&address, &hold)).expect("connected builder");
    assert!(hold.lock().unwrap().socket.is_some());
    let _other = futures_lite::future::block_on(builder_with("tcp:host=127.0.0.1,port=9", &hold))
        .expect("zbus builder");
    assert!(hold.lock().unwrap().socket.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}
