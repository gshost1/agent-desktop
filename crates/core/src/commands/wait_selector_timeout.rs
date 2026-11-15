//! The `TIMEOUT` a selector wait returns once its budget runs out, with the
//! last tree it saw saved as a snapshot.

use std::time::Duration;

use serde_json::Value;

use super::{WaitSelectorInput, build_snapshot, poll_error_json};
use crate::{
    AppError, PlatformAdapter, commands::wait_timeout, context::CommandContext,
    refs::refs_store::RefStore, snapshot, trace::trace_artifacts,
};

const DIAGNOSTIC_SNAPSHOT_BUDGET: Duration = Duration::from_millis(600);

pub(super) fn timeout_response(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    context: &CommandContext,
    previous: (Option<snapshot::SnapshotResult>, Option<Value>),
) -> Result<Value, AppError> {
    let (mut last_built, mut last_error) = previous;
    if last_built.is_none() {
        let diagnostic_deadline =
            crate::Deadline::detached_after(DIAGNOSTIC_SNAPSHOT_BUDGET.as_millis() as u64)?;
        match build_snapshot(adapter, input, entry, diagnostic_deadline) {
            Ok(result) => last_built = Some(result),
            Err(error) if last_error.is_none() => last_error = Some(poll_error_json(&error)),
            Err(_) => {}
        }
    }
    let snapshot_id = persist_last_built(context, last_built.as_mut())?;
    wait_timeout::selector(
        &input.query_raw,
        input.gone,
        input.timeout_ms,
        last_error,
        snapshot_id,
    )
}

fn persist_last_built(
    context: &CommandContext,
    last_built: Option<&mut snapshot::SnapshotResult>,
) -> Result<Option<String>, AppError> {
    let Some(result) = last_built else {
        return Ok(None);
    };
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = result.save_refmap(&store)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id, &result.refmap)?;
    Ok(Some(snapshot_id))
}
