//! Where a daemon listens: `<state root>/daemon/<key>.sock`, with its lock and state files beside
//! it. The key hashes everything a daemon is specific to, so each user, build, session, agent id
//! and environment gets its own process.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use super::private_dir::check_private_dir;
use super::request::{DaemonRequest, PROTOCOL_VERSION};

const DIR_NAME: &str = "daemon";
const SOCKET_SUFFIX: &str = ".sock";
const KEY_LEN: usize = 16;
/// `sun_path` holds 104 bytes on macOS and 108 on Linux, including the terminator.
const SOCKET_PATH_LIMIT: usize = 100;
const ROUTE_FLAGS: [&str; 2] = ["--session", "--agent-id"];

/// One daemon's directory and key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DaemonEndpoint {
    dir: PathBuf,
    key: String,
}

impl DaemonEndpoint {
    /// The endpoint that serves `request`, or `None` when the state root cannot be resolved or
    /// the socket path would not fit.
    pub(crate) fn for_request(request: &DaemonRequest) -> Option<Self> {
        let dir = state_root()?.join(DIR_NAME);
        Self::new(dir, key_for(request)).filter(|endpoint| endpoint.fits())
    }

    /// The endpoint a daemon was started for, from the socket path it was given.
    pub(crate) fn from_socket(socket: &Path) -> Option<Self> {
        let dir = socket.parent()?.to_path_buf();
        let name = socket.file_name()?.to_str()?;
        let key = name.strip_suffix(SOCKET_SUFFIX)?;
        if dir.file_name()?.to_str()? != DIR_NAME {
            return None;
        }
        Self::new(dir, key.to_owned()).filter(|endpoint| endpoint.fits())
    }

    fn new(dir: PathBuf, key: String) -> Option<Self> {
        let well_formed = key.len() == KEY_LEN && key.bytes().all(|byte| byte.is_ascii_hexdigit());
        (well_formed && dir.is_absolute()).then_some(Self { dir, key })
    }

    fn fits(&self) -> bool {
        self.socket().as_os_str().len() <= SOCKET_PATH_LIMIT
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn socket(&self) -> PathBuf {
        self.dir.join(format!("{}{SOCKET_SUFFIX}", self.key))
    }

    pub(crate) fn lock(&self) -> PathBuf {
        self.dir.join(format!("{}.lock", self.key))
    }

    pub(crate) fn state(&self) -> PathBuf {
        self.dir.join(format!("{}.state", self.key))
    }

    pub(crate) fn capture(&self, stream: &str) -> PathBuf {
        self.dir
            .join(format!("{}.{stream}.{}", self.key, std::process::id()))
    }

    /// Whether the directory is still private to this user.
    pub(crate) fn is_private(&self) -> bool {
        check_private_dir(&self.dir).is_ok()
    }
}

/// The state root as core resolves it: a valid `AGENT_DESKTOP_HOME`, else `$HOME/.agent-desktop`.
fn state_root() -> Option<PathBuf> {
    agent_desktop_core::validate_state_root_env().ok()?;
    if let Some(root) = std::env::var_os("AGENT_DESKTOP_HOME") {
        return Some(PathBuf::from(root));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    home.is_absolute().then(|| home.join(".agent-desktop"))
}

fn key_for(request: &DaemonRequest) -> String {
    let mut hasher = DefaultHasher::new();
    PROTOCOL_VERSION.hash(&mut hasher);
    request.build.hash(&mut hasher);
    unsafe { libc::geteuid() }.hash(&mut hasher);
    route_values(&request.argv).hash(&mut hasher);
    request.env.pairs().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// The `--session` and `--agent-id` values in `argv`. They only pick which daemon a call goes
/// to: the daemon parses the arguments itself, so a value misread here costs a separate daemon,
/// never a command run in the wrong session.
pub(crate) fn route_values(argv: &[String]) -> Vec<&str> {
    let mut values = Vec::new();
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        for flag in ROUTE_FLAGS {
            if arg == flag {
                values.push(args.next().map_or("", String::as_str));
            } else if let Some(value) = arg.strip_prefix(flag).and_then(|v| v.strip_prefix('=')) {
                values.push(value);
            }
        }
    }
    values
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
