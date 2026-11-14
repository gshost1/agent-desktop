//! What a client sends: its arguments and everything about its process that a command can
//! depend on, so the daemon runs the command as this process would have.

use super::build_identity::BuildIdentity;
use super::env_subset::EnvSubset;

/// Bumped whenever a message changes shape; part of every daemon key.
pub(crate) const PROTOCOL_VERSION: u32 = 1;

/// One command for the daemon to run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DaemonRequest {
    pub(crate) protocol: u32,
    pub(crate) build: String,
    pub(crate) argv: Vec<String>,
    pub(crate) cwd: String,
    pub(crate) umask: u32,
    pub(crate) env: EnvSubset,
}

impl DaemonRequest {
    /// This process's request; `None` when an argument, the working directory or a relevant
    /// variable is not valid UTF-8, or the build cannot be identified.
    pub(crate) fn current() -> Option<Self> {
        let argv = std::env::args_os()
            .map(|arg| arg.into_string().ok())
            .collect::<Option<Vec<_>>>()?;
        let cwd = std::env::current_dir().ok()?.into_os_string().into_string().ok()?;
        Some(Self {
            protocol: PROTOCOL_VERSION,
            build: BuildIdentity::current()?.as_str().to_owned(),
            argv,
            cwd,
            umask: current_umask(),
            env: EnvSubset::current()?,
        })
    }
}

/// The file-creation mask. Reading it means setting it, so it is put straight back.
#[allow(clippy::useless_conversion)]
#[allow(clippy::useless_conversion)]
pub(crate) fn current_umask() -> u32 {
    let mask = unsafe { libc::umask(0o077) };
    unsafe { libc::umask(mask) };
    u32::from(mask)
}
