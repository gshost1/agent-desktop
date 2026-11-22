use super::{exit_byte, reply, run_until_idle};
use crate::daemon::build_identity::BuildIdentity;
use crate::daemon::clock::Clock;
use crate::daemon::endpoint::DaemonEndpoint;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::idle_policy::IdlePolicy;
use crate::daemon::job::SharedWriter;
use crate::daemon::server_state::ServerState;
use crate::daemon::state_file::DaemonStateFile;
use crate::daemon::transport::DaemonStream;
use std::cell::Cell;
use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A virtual clock that moves `step` every time it is read, counting its reads.
struct SteppingClock {
    now: Cell<Instant>,
    step: Duration,
    reads: Cell<u32>,
}

impl Clock for SteppingClock {
    fn now(&self) -> Instant {
        self.reads.set(self.reads.get() + 1);
        let now = self.now.get();
        self.now.set(now + self.step);
        now
    }
}

fn server() -> ServerState {
    let metadata = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    ServerState::new(
        DaemonEndpoint::from_socket(std::path::Path::new("/r/daemon/0123456789abcdef.sock"))
            .unwrap(),
        BuildIdentity::from_parts("9.9.9", &metadata),
        EnvSubset::from_pairs(Vec::new()),
    )
}

#[test]
fn exit_statuses_map_to_their_bytes() {
    assert_eq!(exit_byte(ExitCode::SUCCESS), 0);
    assert_eq!(exit_byte(ExitCode::FAILURE), 1);
    assert_eq!(exit_byte(ExitCode::from(2)), 2);
}

#[test]
fn an_idle_daemon_exits_after_its_limit_on_the_virtual_clock_and_takes_its_slot() {
    let dir = std::env::temp_dir().join(format!("ad-executor-idle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let record = DaemonStateFile::create(&dir.join("k.state")).unwrap();
    let server = server();
    let (_jobs, queue) = channel();
    let clock = SteppingClock {
        now: Cell::new(Instant::now()),
        step: Duration::from_secs(60),
        reads: Cell::new(0),
    };
    let policy = IdlePolicy {
        idle: Duration::from_secs(300),
        tick: Duration::from_millis(1),
    };
    let started = Instant::now();
    run_until_idle(&queue, &server, &record, policy, &clock);
    let real = started.elapsed();
    let slot_free = server.claim();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(
        clock.reads.get(),
        6,
        "one start read, then five idle minutes"
    );
    assert!(
        real < Duration::from_secs(5),
        "virtual minutes pass in {real:?}"
    );
    assert!(
        !slot_free,
        "an exiting daemon keeps its slot so no command slips in"
    );
}

#[test]
fn a_daemon_whose_listener_is_gone_exits_at_once() {
    let dir = std::env::temp_dir().join(format!("ad-executor-gone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let record = DaemonStateFile::create(&dir.join("k.state")).unwrap();
    let (jobs, queue) = channel();
    drop(jobs);
    let clock = SteppingClock {
        now: Cell::new(Instant::now()),
        step: Duration::ZERO,
        reads: Cell::new(0),
    };
    run_until_idle(
        &queue,
        &server(),
        &record,
        IdlePolicy::new(Duration::from_secs(300)),
        &clock,
    );
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(clock.reads.get(), 1);
}

struct AdmissionCheckingStream {
    server: Arc<ServerState>,
    fail: bool,
}

impl Read for AdmissionCheckingStream {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        unreachable!()
    }
}

impl Write for AdmissionCheckingStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        assert!(
            self.server.claim(),
            "the next call can enter before any reply bytes"
        );
        self.server.release();
        if self.fail {
            Err(std::io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(bytes.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl DaemonStream for AdmissionCheckingStream {
    fn try_clone_stream(&self) -> std::io::Result<Box<dyn DaemonStream>> {
        unreachable!()
    }

    fn set_read_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }

    fn set_write_timeout(&self, _: Option<Duration>) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn replies_release_admission_before_writing_even_when_empty_or_disconnected() {
    for (stdout, fail) in [
        (b"result".to_vec(), false),
        (Vec::new(), false),
        (Vec::new(), true),
    ] {
        let server = Arc::new(server());
        assert!(server.claim());
        let writer: SharedWriter = Arc::new(Mutex::new(Box::new(AdmissionCheckingStream {
            server: Arc::clone(&server),
            fail,
        })));

        reply(&writer, 0, (stdout, Vec::new()), &server);

        assert!(server.claim());
    }
}
