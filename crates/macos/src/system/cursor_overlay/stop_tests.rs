use super::*;
use agent_desktop_core::ErrorCode;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn socket(name: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    PathBuf::from(format!(
        "/tmp/cs-{}-{unique:x}-{name}.sock",
        std::process::id()
    ))
}

fn take_control(listener: &UnixListener) -> UnixStream {
    let (mut stream, _) = listener.accept().unwrap();
    let mut payload = Vec::new();
    stream.read_to_end(&mut payload).unwrap();
    assert!(!payload.is_empty(), "the stop must reach the overlay");
    stream
}

fn stop(path: &Path, budget: Duration) -> Result<(), AdapterError> {
    let control = CursorOverlayControl::disable("run-stop".into());
    stop_until(path, &control, Instant::now() + budget)
}

#[test]
fn an_overlay_that_exits_before_acknowledging_the_stop_is_stopped() {
    let path = socket("exit");
    let listener = UnixListener::bind(&path).unwrap();
    let overlay_path = path.clone();
    let overlay = thread::spawn(move || {
        drop(take_control(&listener));
        std::fs::remove_file(&overlay_path).unwrap();
    });

    let result = stop(&path, Duration::from_secs(2));

    overlay.join().unwrap();
    let _ = std::fs::remove_file(&path);
    result.expect("an overlay that is already gone needs no acknowledgement");
}

#[test]
fn an_overlay_that_dies_with_its_socket_left_behind_is_stopped() {
    let path = socket("dead");
    let listener = UnixListener::bind(&path).unwrap();
    let overlay = thread::spawn(move || drop(take_control(&listener)));

    let result = stop(&path, Duration::from_secs(2));

    overlay.join().unwrap();
    let _ = std::fs::remove_file(&path);
    result.expect("a socket nothing listens on is an overlay that already stopped");
}

#[test]
fn a_live_overlay_that_drops_one_stop_and_answers_the_next_is_stopped() {
    let path = socket("again");
    let listener = UnixListener::bind(&path).unwrap();
    let overlay = thread::spawn(move || {
        drop(take_control(&listener));
        listener.set_nonblocking(true).unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until {
            if let Ok((stream, _)) = listener.accept() {
                stream.set_nonblocking(false).unwrap();
                let mut payload = Vec::new();
                (&stream).read_to_end(&mut payload).unwrap();
                (&stream).write_all(&[1]).unwrap();
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }
    });

    let result = stop(&path, Duration::from_secs(2));

    overlay.join().unwrap();
    let _ = std::fs::remove_file(&path);
    result.expect("the overlay acknowledged the stop it kept");
}

#[test]
fn a_live_overlay_that_keeps_dropping_the_stop_times_out() {
    let path = socket("drops");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let running = Arc::clone(&done);
    let overlay = thread::spawn(move || {
        while !running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    let mut payload = Vec::new();
                    let _ = (&stream).read_to_end(&mut payload);
                }
                Err(_) => thread::sleep(Duration::from_millis(2)),
            }
        }
    });

    let started = Instant::now();
    let error = stop(&path, Duration::from_millis(150)).expect_err("the overlay never stopped");

    done.store(true, Ordering::SeqCst);
    overlay.join().unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn a_live_overlay_that_holds_the_stop_unanswered_times_out() {
    let path = socket("holds");
    let listener = UnixListener::bind(&path).unwrap();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let overlay = thread::spawn(move || {
        let stream = take_control(&listener);
        let _ = released.recv_timeout(Duration::from_secs(10));
        drop(stream);
    });

    let error = stop(&path, Duration::from_millis(100)).expect_err("the stop was never answered");

    release.send(()).unwrap();
    overlay.join().unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(error.code, ErrorCode::Timeout);
}
