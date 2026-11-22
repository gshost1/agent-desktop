//! The `--daemon` global flag.

use clap::Args;

/// Opts one invocation into daemon mode, as `AGENT_DESKTOP_DAEMON=1` does for every call.
#[derive(Args, Debug, Default)]
pub(crate) struct DaemonArgs {
    #[arg(
        long,
        global = true,
        help = "Run through a warm per-session background process (same output); also AGENT_DESKTOP_DAEMON=1"
    )]
    pub daemon: bool,
}
