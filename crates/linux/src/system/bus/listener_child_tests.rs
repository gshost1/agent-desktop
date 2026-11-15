use std::collections::BTreeMap;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use super::fork_holder;

fn finish(pid: libc::pid_t) {
    unsafe {
        libc::kill(pid, libc::SIGKILL);
        libc::waitpid(pid, std::ptr::null_mut(), 0);
    }
}

fn descriptors(pid: libc::pid_t) -> BTreeMap<i32, String> {
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let fd = entry.file_name().to_str()?.parse().ok()?;
                    let target = std::fs::read_link(entry.path()).ok()?;
                    Some((fd, target.display().to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_holder_keeps_the_socket_open_after_the_caller_drops_it() {
    let (held, mut peer) = UnixStream::pair().unwrap();
    let child = fork_holder(held.as_raw_fd(), Duration::from_millis(400))
        .unwrap()
        .pid();
    drop(held);
    let started = Instant::now();
    peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut byte = [0u8; 1];
    assert_eq!(
        peer.read(&mut byte).unwrap(),
        0,
        "the socket closes once held"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "the socket closed after {:?}, before the hold ended",
        started.elapsed()
    );
    finish(child);
}

#[test]
fn the_holder_keeps_only_null_standard_streams_and_the_socket() {
    let (held, _peer) = UnixStream::pair().unwrap();
    let (_stray, _stray_peer) = UnixStream::pair().unwrap();
    let socket = std::fs::read_link(format!("/proc/self/fd/{}", held.as_raw_fd())).unwrap();
    let child = fork_holder(held.as_raw_fd(), Duration::from_secs(5))
        .unwrap()
        .pid();
    let wanted: BTreeMap<i32, String> = [
        (0, "/dev/null".to_owned()),
        (1, "/dev/null".to_owned()),
        (2, "/dev/null".to_owned()),
        (3, socket.display().to_string()),
    ]
    .into();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut seen = descriptors(child);
    while seen != wanted && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
        seen = descriptors(child);
    }
    finish(child);
    assert_eq!(seen, wanted);
}

#[test]
fn the_fallback_close_loop_is_bounded_when_the_limit_is_infinite() {
    assert_eq!(super::close_ceiling(libc::RLIM_INFINITY), 1 << 20);
    assert_eq!(super::close_ceiling(1024), 1024);
}
