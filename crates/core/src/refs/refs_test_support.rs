use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct HomeGuard {
    _dir: TempDir,
    prev: Option<PathBuf>,
}

impl HomeGuard {
    pub(crate) fn new() -> Self {
        let dir = TempDir::new();
        let prev = crate::refs::set_home_override(Some(dir.path().to_path_buf()));
        Self { _dir: dir, prev }
    }

    pub(crate) fn path(&self) -> &Path {
        self._dir.path()
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        crate::refs::set_home_override(prev);
    }
}

pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("agent-desktop-test-{nanos}-{n}"));
        fs::create_dir_all(&path).expect("create tempdir");
        Self(path)
    }

    pub(crate) fn adopt(path: PathBuf) -> Self {
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if fs::remove_dir_all(&self.0).is_err() {
            let _ = fs::remove_file(&self.0);
        }
    }
}

const SUBPROCESS_READY_MARKER: &str = "agent-desktop-test-subprocess-ready";

/// How long a test waits on a helper subprocess before it calls the helper
/// hung. Only a real hang reaches it: it is not a latency budget, so a
/// scheduler that starves the helper for seconds still passes.
pub(crate) const SUBPROCESS_HANG_LIMIT: Duration = Duration::from_secs(120);

/// A helper subprocess that reports through its stdout when it reached its
/// rendezvous point and then blocks until the test releases it through its
/// stdin, so the test waits on the event itself instead of a wall-clock budget.
pub(crate) struct HandshakeChild {
    child: Child,
    stdin: Option<ChildStdin>,
    ready: Receiver<bool>,
}

impl HandshakeChild {
    pub(crate) fn spawn(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (sender, ready) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut signalled = false;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if !signalled && line.contains(SUBPROCESS_READY_MARKER) {
                    signalled = true;
                    let _ = sender.send(true);
                }
            }
            if !signalled {
                let _ = sender.send(false);
            }
        });
        Self {
            child,
            stdin,
            ready,
        }
    }

    pub(crate) fn wait_ready(&mut self) {
        match self.ready.recv_timeout(SUBPROCESS_HANG_LIMIT) {
            Ok(true) => {}
            Ok(false) | Err(RecvTimeoutError::Disconnected) => {
                let status = self.child.wait();
                panic!("subprocess exited before it reached its rendezvous: {status:?}");
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                panic!("subprocess hung before it reached its rendezvous");
            }
        }
    }

    pub(crate) fn release(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"release\n");
        }
    }

    pub(crate) fn wait_exit(&mut self) -> ExitStatus {
        let hang_at = Instant::now() + SUBPROCESS_HANG_LIMIT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            if Instant::now() >= hang_at {
                let _ = self.child.kill();
                panic!("subprocess hung after it was released");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(crate) fn kill(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}

impl Drop for HandshakeChild {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The subprocess side of [`HandshakeChild`]: reports the rendezvous, then
/// blocks until the test releases it or exits.
pub(crate) fn signal_ready_and_wait_for_release() {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(format!("\n{SUBPROCESS_READY_MARKER}\n").as_bytes())
        .unwrap();
    stdout.flush().unwrap();
    drop(stdout);
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
}

#[cfg(test)]
#[path = "refs_test_support_tests.rs"]
mod tests;
