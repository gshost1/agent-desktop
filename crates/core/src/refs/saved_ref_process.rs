//! The process a saved ref was captured from, read for callers that only hint
//! at work the ref's command is about to do.

use crate::{ProcessId, refs::refs_store::RefStore};

/// Returns the process recorded for `ref_id` in its snapshot, or `None` when
/// the ref, the session or the snapshot cannot be read. It records no trace
/// and reports no error: the command that owns the ref still loads and checks
/// it, and this answer only steers a best-effort hint.
pub fn saved_ref_process(ref_id: &str, session_id: Option<&str>) -> Option<ProcessId> {
    let (snapshot_id, local_ref) = crate::refs::ref_token::resolve_ref_target(ref_id).ok()?;
    let refmap = RefStore::for_session(session_id)
        .ok()?
        .load_snapshot(&snapshot_id)
        .ok()?;
    refmap.get(&local_ref).map(|entry| entry.process.pid)
}

#[cfg(test)]
#[path = "saved_ref_process_tests.rs"]
mod tests;
