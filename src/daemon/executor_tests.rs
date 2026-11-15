use super::{exit_byte, run_until_idle};
use crate::daemon::build_identity::BuildIdentity;
use crate::daemon::clock::Clock;
use crate::daemon::endpoint::DaemonEndpoint;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::idle_policy::IdlePolicy;
use crate::daemon::server_state::ServerState;
use crate::daemon::state_file::DaemonStateFile;
use std::cell::Cell;
use std::process::ExitCode;
use std::sync::mpsc::channel;
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
