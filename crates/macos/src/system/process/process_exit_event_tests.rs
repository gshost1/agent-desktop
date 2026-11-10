use super::*;
use std::time::Instant;

#[test]
fn a_wait_ends_when_the_process_exits_not_when_the_pause_does() {
    let mut child = std::process::Command::new("/bin/sleep")
        .arg("0.2")
        .spawn()
        .expect("spawn sleep");
    let pid = i32::try_from(child.id()).expect("pid fits");
    let started = Instant::now();
    wait_for_exit_event(pid, Duration::from_secs(5));
    let waited = started.elapsed();
    let _ = child.wait();
    assert!(
        waited >= Duration::from_millis(150),
        "woke early: {waited:?}"
    );
    assert!(
        waited < Duration::from_secs(2),
        "woke at the pause: {waited:?}"
    );
}

#[test]
fn a_process_that_is_already_gone_ends_the_wait_at_once() {
    let mut child = std::process::Command::new("/usr/bin/true")
        .spawn()
        .expect("spawn true");
    let pid = i32::try_from(child.id()).expect("pid fits");
    let _ = child.wait();
    let started = Instant::now();
    wait_for_exit_event(pid, Duration::from_secs(5));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_pid_the_kernel_cannot_watch_sleeps_the_whole_pause() {
    let started = Instant::now();
    wait_for_exit_event(-1, Duration::from_millis(60));
    assert!(started.elapsed() >= Duration::from_millis(60));
}
