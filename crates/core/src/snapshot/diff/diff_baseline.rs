use std::io::{BufWriter, ErrorKind, Write};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::json;

use crate::{
    AccessibilityNode, AdapterError, AppError, DeliverySemantics, ErrorCode,
    adapter::TreeOptions,
    refs::{refs_store::RefStore, validate_snapshot_id},
    snapshot::SnapshotResult,
    snapshot::diff::{
        baseline_view::BaselineView, capped_writer::CappedWriter, diff_owner::DiffOwner,
        diff_presentation::DiffPresentation, row_view::RowSeq,
    },
};

const BASELINE_FILE: &str = "diff_baseline.json";
const BASELINE_FORMAT: u32 = 1;

/// The largest baseline written or read, the refmap's own cap. A tree whose
/// rows do not fit gets no baseline, and a later `--since` naming it is
/// refused as missing one.
pub(crate) const MAX_BASELINE_BYTES: usize = crate::refs::MAX_REFMAP_BYTES as usize;

/// What a later `snapshot --since` compares against: the rows of the tree a
/// snapshot presented, with what it observed and how it shaped the tree. It is
/// saved beside the snapshot's refmap, in the same session namespace, and is
/// pruned with it.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DiffBaseline {
    pub(crate) format: u32,
    pub(crate) owner: DiffOwner,
    pub(crate) presentation: DiffPresentation,
    pub(crate) complete: bool,
    pub(crate) rows: Vec<AccessibilityNode>,
}

/// Saves the baseline of a snapshot that was just saved as `snapshot_id`,
/// streaming its rows from the tree into a new private file.
///
/// The write is not flushed to the device and not replaced atomically: the
/// snapshot id is not printed until it returns, so nothing reads the file
/// while it is written, and a file cut short by a crash does not parse and is
/// refused as missing. A baseline that would pass `MAX_BASELINE_BYTES` stops
/// at the cap and is removed; the snapshot itself is unaffected and only a
/// diff against it is refused.
pub(crate) fn save_baseline(
    store: &RefStore,
    snapshot_id: &str,
    result: &SnapshotResult,
    opts: &TreeOptions,
) -> Result<(), AppError> {
    validate_snapshot_id(snapshot_id)?;
    let path = baseline_path(store, snapshot_id);
    let view = BaselineView {
        format: BASELINE_FORMAT,
        owner: DiffOwner::of(&result.owner),
        presentation: DiffPresentation::of(opts),
        complete: result.complete,
        rows: RowSeq(&result.tree),
    };
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    let file = crate::storage::private_file::open_private_append(&path)?;
    let mut writer = CappedWriter::new(BufWriter::new(file), MAX_BASELINE_BYTES);
    let written = serde_json::to_writer(&mut writer, &view)
        .map_err(std::io::Error::from)
        .and_then(|()| writer.flush());
    drop(writer.into_inner());
    match written {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&path);
            if err.kind() == ErrorKind::FileTooLarge {
                tracing::debug!(snapshot_id, "snapshot: diff baseline over its cap, not saved");
                return Ok(());
            }
            Err(err.into())
        }
    }
}

/// Loads the baseline of `snapshot_id` from `store`'s session namespace only.
/// An unknown or pruned snapshot, and one saved without a baseline, both fail
/// `SNAPSHOT_NOT_FOUND`, with `details.kind` telling them apart.
pub(crate) fn load_baseline(store: &RefStore, snapshot_id: &str) -> Result<DiffBaseline, AppError> {
    validate_snapshot_id(snapshot_id)?;
    let path = baseline_path(store, snapshot_id);
    let bytes = match crate::storage::private_file::read_private_bounded(
        &path,
        MAX_BASELINE_BYTES as u64,
    ) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(missing(store, snapshot_id));
        }
        Err(err) => return Err(err.into()),
    };
    match serde_json::from_slice::<DiffBaseline>(&bytes) {
        Ok(baseline) if baseline.format == BASELINE_FORMAT => Ok(baseline),
        _ => Err(not_found(snapshot_id, "diff_baseline_missing")),
    }
}

fn missing(store: &RefStore, snapshot_id: &str) -> AppError {
    let refmap_saved = snapshot_dir(store, snapshot_id)
        .join("refmap.json")
        .is_file();
    let kind = if refmap_saved {
        "diff_baseline_missing"
    } else {
        "since_snapshot_not_found"
    };
    not_found(snapshot_id, kind)
}

fn not_found(snapshot_id: &str, kind: &str) -> AppError {
    let message = match kind {
        "diff_baseline_missing" => {
            format!("Snapshot '{snapshot_id}' has no saved tree to compare against")
        }
        _ => format!("Snapshot '{snapshot_id}' not found in this session"),
    };
    AppError::Adapter(
        AdapterError::new(ErrorCode::SnapshotNotFound, message)
            .with_suggestion(
                "Take a full snapshot without --since, read its tree, and pass its snapshot_id to \
                 --since after the next action. --since compares only against a snapshot taken by \
                 the snapshot command under the same --session or AGENT_DESKTOP_SESSION, until the \
                 store prunes it",
            )
            .with_details(json!({ "kind": kind, "since": snapshot_id }))
            .with_disposition(DeliverySemantics::not_delivered()),
    )
}

fn baseline_path(store: &RefStore, snapshot_id: &str) -> PathBuf {
    snapshot_dir(store, snapshot_id).join(BASELINE_FILE)
}

fn snapshot_dir(store: &RefStore, snapshot_id: &str) -> PathBuf {
    store.base_dir().join("snapshots").join(snapshot_id)
}

#[cfg(test)]
#[path = "diff_baseline_tests.rs"]
mod tests;
