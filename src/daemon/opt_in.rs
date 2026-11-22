//! Whether this invocation asked for daemon mode: `AGENT_DESKTOP_DAEMON=1` or `--daemon`. A
//! call that inherited an interaction lease from an MCP server always runs in its own process,
//! because only that process holds the lease's descriptor or handle.

use std::ffi::OsStr;

use super::OPT_IN_ENV;

const FLAG: &str = "--daemon";

/// Whether this process should try its daemon before running the command itself.
pub(crate) fn requested() -> bool {
    let env = std::env::var_os(OPT_IN_ENV);
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    asked(env.as_deref(), args.iter().map(|arg| arg.as_os_str())) && !inherits_interaction_lease()
}

/// Whether `env` (the opt-in variable) or one of `args` (program name excluded) asks for the
/// daemon. The flag is looked for up to `--`: a value that happens to read `--daemon` only
/// routes the call through the daemon, which parses the arguments exactly as this process would.
pub(crate) fn asked<'a>(env: Option<&OsStr>, mut args: impl Iterator<Item = &'a OsStr>) -> bool {
    env.is_some_and(|value| value == "1")
        || args
            .by_ref()
            .take_while(|arg| *arg != "--")
            .any(|arg| arg == FLAG)
}

fn inherits_interaction_lease() -> bool {
    #[cfg(unix)]
    let variable = agent_desktop_core::INTERACTION_LEASE_FD_ENV;
    #[cfg(not(unix))]
    let variable = agent_desktop_core::INTERACTION_LEASE_HANDLE_ENV;
    std::env::var_os(variable).is_some()
}

#[cfg(test)]
#[path = "opt_in_tests.rs"]
mod tests;
