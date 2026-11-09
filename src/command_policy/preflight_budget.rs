//! The deadline of the permission report a command reads before it runs.

use agent_desktop_core::{AdapterError, DEFAULT_OPERATION_TIMEOUT_MS, Deadline};

use crate::cli::Commands;

/// Cap on a top-level batch's permission preflight, which may outlast five seconds.
const BATCH_PREFLIGHT_CAP_MS: u64 = 15_000;

/// How far past its own timeout a command's permission read may run. The read
/// happens before the command's deadline starts, so a read that never answers
/// must end within the one second a command may overrun its timeout by, while
/// a healthy read, a cold bus connection included, always gets this long.
const PREFLIGHT_SLACK_MS: u64 = 900;

/// The preflight deadline: the caller's, a batch's capped budget, else the
/// standard one, cut to the command's own timeout plus [`PREFLIGHT_SLACK_MS`]
/// when that is shorter.
pub(crate) fn preflight_deadline(
    cmd: &Commands,
    deadline: Option<Deadline>,
) -> Result<Deadline, AdapterError> {
    match (cmd, deadline) {
        (_, Some(deadline)) => Ok(deadline),
        (Commands::Batch(a), None) => Deadline::after(a.timeout_ms.min(BATCH_PREFLIGHT_CAP_MS)),
        (_, None) => Deadline::after(own_timeout_ms(cmd).map_or(
            DEFAULT_OPERATION_TIMEOUT_MS,
            |timeout_ms| {
                timeout_ms
                    .saturating_add(PREFLIGHT_SLACK_MS)
                    .min(DEFAULT_OPERATION_TIMEOUT_MS)
            },
        )),
    }
}

fn own_timeout_ms(cmd: &Commands) -> Option<u64> {
    match cmd {
        Commands::Snapshot(a) => Some(
            a.timeout_ms
                .unwrap_or(agent_desktop_core::snapshot::DEFAULT_SNAPSHOT_TIMEOUT_MS),
        ),
        Commands::Find(a) => a.timeout_ms,
        Commands::Click(a)
        | Commands::DoubleClick(a)
        | Commands::TripleClick(a)
        | Commands::RightClick(a)
        | Commands::Clear(a)
        | Commands::Focus(a)
        | Commands::Toggle(a)
        | Commands::Check(a)
        | Commands::Uncheck(a)
        | Commands::Expand(a)
        | Commands::Collapse(a)
        | Commands::ScrollTo(a) => Some(a.timeout_ms),
        Commands::Type(a) => Some(a.effective_timeout_ms()),
        Commands::SetValue(a) => Some(a.timeout_ms),
        Commands::Select(a) => Some(a.timeout_ms),
        Commands::Scroll(a) => Some(a.timeout_ms),
        Commands::Hover(a) => Some(a.timeout_ms),
        Commands::Drag(a) => Some(a.timeout_ms),
        Commands::OpenUrl(a) => a.timeout_ms,
        Commands::Wait(a) => Some(a.timeout),
        Commands::Launch(a) => Some(a.timeout),
        _ => None,
    }
}

#[cfg(test)]
#[path = "preflight_budget_tests.rs"]
mod tests;
