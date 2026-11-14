//! A permission refusal raised before dispatch may belong to the daemon alone: on macOS the
//! Accessibility and Screen Recording grants follow the process that started it, which need not
//! be the caller's terminal. Such a result is discarded and the caller runs the command itself,
//! which is safe because nothing was delivered.

const INSPECTED_BYTES: usize = 64 * 1024;

/// Whether `stdout` holds a `PERM_DENIED` envelope that delivered nothing.
pub(crate) fn is_undelivered_permission_refusal(stdout: &[u8]) -> bool {
    if stdout.len() > INSPECTED_BYTES {
        return false;
    }
    let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(stdout) else {
        return false;
    };
    let error = &envelope["error"];
    envelope["ok"] == false
        && error["code"] == "PERM_DENIED"
        && error["disposition"]["delivery"] == "not_delivered"
}

#[cfg(test)]
#[path = "rerun_tests.rs"]
mod tests;
