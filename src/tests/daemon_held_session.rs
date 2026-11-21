use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

use super::support::Home;

pub(super) struct HeldSession {
    pub(super) id: String,
    pub(super) pid: i32,
    _lock: File,
}

impl HeldSession {
    pub(super) fn new(home: &Home) -> Self {
        let output = home.run(false, &["session", "start", "--no-trace"]);
        assert!(output.status.success(), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let id = value["data"]["session_id"].as_str().unwrap().to_owned();
        let pid = home.warm_for(&["version", "--session", &id]);
        let path = home
            .state_root()
            .join("sessions")
            .join(&id)
            .join("refstore.lock");
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .unwrap();
        assert_eq!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        Self {
            id,
            pid,
            _lock: lock,
        }
    }

    pub(super) fn read_args(&self) -> [&str; 4] {
        ["session", "list", "--session", &self.id]
    }

    pub(super) fn input_args(&self) -> [&str; 5] {
        ["session", "end", &self.id, "--session", &self.id]
    }
}
