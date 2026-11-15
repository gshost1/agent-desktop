use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::HolderHandle;
use crate::system::bus::listener_child::fork_holder;

const CHILD: &str = "AGENT_DESKTOP_HOLDER_TEST_CHILD";

fn reaped(pid: libc::pid_t) -> bool {
    unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) == -1 }
}

fn in_child_process(name: &str, scenario: impl FnOnce()) {
    if std::env::var_os(CHILD).is_some() {
        scenario();
        return;
    }
    let filter = format!("system::bus::listener_holder::tests::{name}");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &filter, "--test-threads=1"])
        .env(CHILD, "1")
        .status()
        .unwrap();
    assert!(status.success(), "{name} failed in its own process");
}

fn without_pidfd(handle: HolderHandle) -> HolderHandle {
    HolderHandle {
        pid: handle.pid,
        pidfd: None,
    }
}

#[test]
fn a_running_holder_is_killed_and_reaped() {
    let (held, _peer) = UnixStream::pair().unwrap();
    for pidfd in [true, false] {
        let handle = fork_holder(held.as_raw_fd(), Duration::from_secs(5)).unwrap();
        let handle = if pidfd { handle } else { without_pidfd(handle) };
        let pid = handle.pid();
        assert!(
            handle.retire(),
            "a running holder is killed (pidfd {pidfd})"
        );
        assert!(reaped(pid), "and reaped (pidfd {pidfd})");
    }
}

#[test]
fn a_holder_the_host_already_reaped_gets_no_signal() {
    in_child_process("a_holder_the_host_already_reaped_gets_no_signal", || {
        unsafe {
            libc::signal(libc::SIGCHLD, libc::SIG_IGN);
        }
        let (held, _peer) = UnixStream::pair().unwrap();
        for pidfd in [true, false] {
            let handle = fork_holder(held.as_raw_fd(), Duration::from_millis(50)).unwrap();
            let handle = if pidfd { handle } else { without_pidfd(handle) };
            std::thread::sleep(Duration::from_millis(400));
            assert!(
                !handle.retire(),
                "an exited holder the kernel reaped gets no kill (pidfd {pidfd})"
            );
        }
    });
}

#[test]
fn the_holder_ends_on_sigterm_even_when_the_host_catches_it() {
    in_child_process(
        "the_holder_ends_on_sigterm_even_when_the_host_catches_it",
        || {
            extern "C" fn ignore(_: libc::c_int) {}
            unsafe {
                libc::signal(
                    libc::SIGTERM,
                    ignore as extern "C" fn(libc::c_int) as libc::sighandler_t,
                );
            }
            let (held, _peer) = UnixStream::pair().unwrap();
            let handle = fork_holder(held.as_raw_fd(), Duration::from_secs(5)).unwrap();
            std::thread::sleep(Duration::from_millis(100));
            unsafe {
                libc::kill(handle.pid(), libc::SIGTERM);
            }
            let mut status = 0;
            let mut ended = false;
            for _ in 0..100 {
                if unsafe { libc::waitpid(handle.pid(), &mut status, libc::WNOHANG) }
                    == handle.pid()
                {
                    ended = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(ended, "the holder kept running after SIGTERM");
            assert!(libc::WIFSIGNALED(status) && libc::WTERMSIG(status) == libc::SIGTERM);
        },
    );
}
