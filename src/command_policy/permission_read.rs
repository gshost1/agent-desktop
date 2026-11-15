//! The permission report a command reads before it runs.

use agent_desktop_core::{AppError, PermissionReport, PlatformAdapter};

use super::{preflight_deadline, requires_full_permission_report, requires_permission_report};
use crate::cli::Commands;

/// Reads the report `cmd` needs, or the empty report when it needs none.
/// `status`, `permissions` and `batch` read the full report; every other
/// command reads the preflight report. An adapter that refuses the read
/// instead of reporting, as for a display that cannot be reached, is the
/// command's answer unchanged: only a report that says a permission is denied
/// becomes `PERM_DENIED`, in [`super::preflight`].
pub(crate) fn read_permission_report(
    cmd: &Commands,
    adapter: &dyn PlatformAdapter,
) -> Result<PermissionReport, AppError> {
    if !requires_permission_report(cmd) {
        return Ok(PermissionReport::default());
    }
    let deadline = preflight_deadline(cmd)?;
    if requires_full_permission_report(cmd) {
        adapter.permission_report(deadline)
    } else {
        adapter.preflight_permission_report(deadline)
    }
    .map_err(AppError::from)
}

#[cfg(test)]
#[path = "permission_read_tests.rs"]
mod tests;
