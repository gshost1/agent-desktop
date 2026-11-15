use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A short private state root (socket paths must fit `sun_path`) whose daemons are stopped and
/// whose files are removed on drop.
pub(super) struct Home {
    pub(super) root: PathBuf,
    pub(super) binary: PathBuf,
}

impl Home {
    pub(super) fn new() -> Self {
        let root = PathBuf::from(format!(
            "/tmp/add-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root.join("home"))
            .unwrap();
        Self {
            root,
            binary: PathBuf::from(env!("CARGO_BIN_EXE_agent-desktop")),
        }
    }

    pub(super) fn state_root(&self) -> PathBuf {
        self.root.join("home")
    }

    pub(super) fn command(&self, daemon: bool, args: &[&str]) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .current_dir(&self.root)
            .env("AGENT_DESKTOP_HOME", self.state_root())
            .env("AGENT_DESKTOP_DAEMON_IDLE_MS", "60000")
            .env_remove("AGENT_DESKTOP_SESSION")
            .env_remove("AGENT_DESKTOP_AGENT_ID")
            .env_remove("RUST_LOG")
            .stdin(Stdio::null());
        if daemon {
            command.env("AGENT_DESKTOP_DAEMON", "1");
        } else {
            command.env_remove("AGENT_DESKTOP_DAEMON");
        }
        command
    }

    pub(super) fn run(&self, daemon: bool, args: &[&str]) -> Output {
        self.command(daemon, args).output().unwrap()
    }

    pub(super) fn spawn(&self, daemon: bool, args: &[&str]) -> Child {
        self.command(daemon, args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    /// `(pid, served)` of every live daemon whose state file is present.
    pub(super) fn daemons(&self) -> Vec<(i32, u64)> {
        let Ok(entries) = std::fs::read_dir(self.state_root().join("daemon")) else {
            return Vec::new();
        };
        let mut found: Vec<(i32, u64)> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "state"))
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .filter_map(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .filter_map(|state| {
                let pid = i32::try_from(state["pid"].as_i64()?).ok()?;
                Some((pid, state["served"].as_u64()?))
            })
            .filter(|(pid, _)| is_alive(*pid))
            .collect();
        found.sort_unstable();
        found
    }

    /// Starts this home's daemon with one call and waits until it answers.
    pub(super) fn warm(&self) -> i32 {
        self.run(true, &["version"]);
        let pid = wait_until(Duration::from_secs(10), || {
            self.daemons().first().map(|(pid, _)| *pid)
        })
        .expect("a daemon starts after the first call");
        let before = self.served(pid);
        wait_until(Duration::from_secs(10), || {
            self.run(true, &["version"]);
            (self.served(pid) > before).then_some(())
        })
        .expect("the daemon serves a call");
        pid
    }

    pub(super) fn served(&self, pid: i32) -> u64 {
        self.daemons()
            .into_iter()
            .find(|(found, _)| *found == pid)
            .map_or(0, |(_, served)| served)
    }

    pub(super) fn stop_daemons(&self) {
        for (pid, _) in self.daemons() {
            unsafe { libc::kill(pid, libc::SIGTERM) };
            assert!(wait_gone(pid), "daemon {pid} stops on SIGTERM");
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        for (pid, _) in self.daemons() {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(super) fn wait_until<T>(limit: Duration, mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let started = Instant::now();
    loop {
        if let Some(found) = probe() {
            return Some(found);
        }
        if started.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn is_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

pub(super) fn wait_gone(pid: i32) -> bool {
    wait_until(Duration::from_secs(10), || (!is_alive(pid)).then_some(())).is_some()
}

/// Processes of this user whose environment names `home` as their state root (Linux only;
/// elsewhere the daemons' state files are the record).
pub(super) fn processes_using(home: &Path) -> Vec<i32> {
    #[cfg(target_os = "linux")]
    {
        let needle = format!("AGENT_DESKTOP_HOME={}", home.display()).into_bytes();
        std::fs::read_dir("/proc")
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
            .filter(|pid| *pid != i32::try_from(std::process::id()).unwrap())
            .filter(|pid| {
                std::fs::read(format!("/proc/{pid}/environ"))
                    .is_ok_and(|environ| environ.split(|b| *b == 0).any(|var| var == needle))
            })
            .filter(|pid| !is_zombie(*pid))
            .collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = home;
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn is_zombie(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .map(|(_, rest)| rest.starts_with('Z'))
        })
        .unwrap_or(true)
}

/// The output with every timing value and generated id replaced, so two runs of the same
/// command compare equal byte for byte when only the clock differs.
pub(super) fn normalized(bytes: &[u8]) -> String {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    mask(&mut value);
    value.to_string()
}

fn mask(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if is_volatile(key) {
                    *field = serde_json::Value::Null;
                } else {
                    mask(field);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(mask),
        _ => {}
    }
}

fn is_volatile(key: &str) -> bool {
    key.ends_with("_ms")
        || key.ends_with("_at")
        || key.ends_with("_us")
        || matches!(
            key,
            "elapsed" | "duration" | "timestamp" | "pid" | "snapshot_id"
        )
}

/// Stderr with the timestamp that starts each log line removed.
fn without_log_timestamps(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| match line.split_once(' ') {
            Some((stamp, rest)) if stamp.starts_with("20") && stamp.ends_with('Z') => rest,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The same bytes and status from both modes, timing aside.
pub(super) fn assert_same(args: &[&str], in_process: &Output, daemon: &Output) {
    assert_eq!(
        in_process.status.code(),
        daemon.status.code(),
        "{args:?} exit code"
    );
    assert_eq!(
        normalized(&in_process.stdout),
        normalized(&daemon.stdout),
        "{args:?} stdout"
    );
    assert_eq!(
        without_log_timestamps(&in_process.stderr),
        without_log_timestamps(&daemon.stderr),
        "{args:?} stderr"
    );
    assert_eq!(
        in_process.stdout.ends_with(b"\n"),
        daemon.stdout.ends_with(b"\n"),
        "{args:?} trailing newline"
    );
}

/// Whether a daemon answer was a permission refusal the client re-ran itself.
pub(super) fn is_permission_rerun(output: &Output) -> bool {
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .is_ok_and(|envelope| envelope["error"]["code"] == "PERM_DENIED")
}

/// Writes a one-ref refmap for `pid` straight into the state root, as `snapshot` would.
pub(super) fn seed_ref(home: &Home, snapshot_id: &str, pid: u32) -> String {
    let entry = serde_json::json!({
        "pid": pid,
        "process_instance": "test-instance",
        "role": "button",
        "name": "Send",
        "capture": "unique",
        "available_actions": ["Click"],
        "path": [0]
    });
    let map = serde_json::json!({ "inner": { "@e1": entry }, "counter": 1 });
    let dir = home.state_root().join("snapshots").join(snapshot_id);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .unwrap();
    std::fs::write(dir.join("refmap.json"), map.to_string()).unwrap();
    format!("@{snapshot_id}:e1")
}

/// The daemon sockets under `home` that still accept a connection, which only a live daemon's
/// listener does. A daemon killed with SIGKILL leaves a dead socket file that the next daemon
/// for that endpoint replaces; that file is not a listener and is not listed.
pub(super) fn sockets_in(home: &Home) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(home.state_root().join("daemon")) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sock"))
        .filter(|path| std::os::unix::net::UnixStream::connect(path).is_ok())
        .map(|path| path.display().to_string())
        .collect()
}
