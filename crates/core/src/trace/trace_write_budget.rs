use crate::refs::refs_lock::{LOCK_TIMEOUT_MS, RefStoreLock};
use crate::{AppError, Deadline, storage::private_file::open_private_lock};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub(crate) const MAX_SESSION_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_SESSION_EVENTS: usize = 100_000;
pub(crate) const MAX_DIRECTORY_ENTRIES: usize = 4_096;
const ARTIFACT_AND_LEDGER_ENTRY_RESERVE: usize = 16;
const WRITER_ENTRY_LIMIT: usize = MAX_DIRECTORY_ENTRIES - ARTIFACT_AND_LEDGER_ENTRY_RESERVE;
const LEDGER: &str = ".trace-usage.json";
const LEDGER_MAX_BYTES: u64 = 256;
const LEDGER_FIELD_WIDTH: usize = 20;
const LEDGER_RECORD_LEN: usize = 3 * LEDGER_FIELD_WIDTH + 4;

/// The shortest lock wait one trace event gets, measured from that event, so
/// the closing record of an action that spent its deadline is not dropped, or
/// a strict delivered action failed, by a brief wait behind another writer.
const TRACE_LOCK_FLOOR_MS: u64 = 250;

pub(crate) struct TraceWriteBudget {
    ledger: File,
    usage: (usize, usize, usize),
    _lock: RefStoreLock,
}

impl TraceWriteBudget {
    pub(crate) fn acquire(directory: &Path) -> Result<Self, AppError> {
        crate::trace::trace_file::ensure_trace_dir(directory)?;
        let lock = RefStoreLock::acquire_with_deadline(
            &directory.join(".trace-budget.lock"),
            trace_lock_deadline()?,
        )?;
        let ledger_path = directory.join(LEDGER);
        let (ledger, usage) = match open_private_lock(&ledger_path, false) {
            Ok(mut ledger) => {
                let bytes = read_ledger(&mut ledger)?;
                let usage = if bytes.is_empty() {
                    initial_usage(directory, true)?
                } else {
                    parse_ledger(&bytes)?
                };
                if bytes.len() == LEDGER_RECORD_LEN {
                    (ledger, usage)
                } else {
                    drop(ledger);
                    (replace_ledger(&ledger_path, usage)?, usage)
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let usage = initial_usage(directory, false)?;
                (replace_ledger(&ledger_path, usage)?, usage)
            }
            Err(error) => return Err(error.into()),
        };
        let mut budget = Self {
            ledger,
            usage,
            _lock: lock,
        };
        let segment = crate::trace::trace_file::segment_path_for_dir(directory);
        match segment.symlink_metadata() {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                budget.usage.2 = budget.usage.2.saturating_add(1);
                budget.persist()?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(budget)
    }

    pub(crate) fn reserve_line(&mut self, bytes: usize) -> Result<(), AppError> {
        self.usage.0 = self.usage.0.saturating_add(bytes);
        self.usage.1 = self.usage.1.saturating_add(1);
        self.persist()
    }

    /// Rewrites the ledger in place with one fixed-width write and no sync.
    /// [`TraceWriteBudget::acquire`] leaves the file holding exactly one
    /// fixed-width record and every encoding has that length, so a killed
    /// writer leaves either the old record or the new one, never a torn mix.
    /// The trace segments are not synced either, so a sync here bought no
    /// durability the trace has, while it held the cross-process lock for the
    /// whole flush.
    fn persist(&mut self) -> Result<(), AppError> {
        if self.usage.0 > MAX_SESSION_BYTES
            || self.usage.1 > MAX_SESSION_EVENTS
            || self.usage.2 > WRITER_ENTRY_LIMIT
        {
            return Err(AppError::invalid_input_with_suggestion(
                "Session trace reached its reader-compatible recording limit",
                "Start a new session to continue recording; the existing trace remains readable.",
            ));
        }
        self.ledger.seek(SeekFrom::Start(0))?;
        self.ledger
            .write_all(encode_ledger(self.usage).as_bytes())?;
        Ok(())
    }
}

/// The lock wait is what the command deadline has left, at most the
/// store-lock bound; when less than [`TRACE_LOCK_FLOOR_MS`] is left, it is
/// that floor from now, so a contended event can outlast its command
/// deadline by at most the floor.
fn trace_lock_deadline() -> Result<Deadline, AppError> {
    let command = Deadline::after(LOCK_TIMEOUT_MS)?;
    let floor = Deadline::detached_after(TRACE_LOCK_FLOOR_MS)?;
    if command.remaining() >= floor.remaining() {
        Ok(command)
    } else {
        Ok(floor)
    }
}

/// Writes a fixed-width record to a temporary file and renames it over the
/// ledger. A compact ledger from an older writer, an empty one left by a
/// killed writer, or a missing one differs in length from the record, so an
/// in-place rewrite could stop after a short write and leave bytes no reader
/// parses, which stops recording for the session. The rename leaves the old
/// ledger or the new one.
fn replace_ledger(path: &Path, usage: (usize, usize, usize)) -> Result<File, AppError> {
    crate::storage::private_file::write_atomic(path, encode_ledger(usage).as_bytes())?;
    Ok(open_private_lock(path, false)?)
}

fn read_ledger(ledger: &mut File) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    ledger
        .take(LEDGER_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LEDGER_MAX_BYTES {
        return Err(AppError::invalid_input_with_suggestion(
            "Session trace recording ledger is larger than its record",
            "Start a new session.",
        ));
    }
    Ok(bytes)
}

fn parse_ledger(bytes: &[u8]) -> Result<(usize, usize, usize), AppError> {
    serde_json::from_slice(bytes).map_err(|_| {
        AppError::invalid_input_with_suggestion(
            "Session trace recording ledger is unreadable, so recording stopped",
            "Start a new session to continue recording; the existing trace remains readable.",
        )
    })
}

fn encode_ledger(usage: (usize, usize, usize)) -> String {
    format!(
        "[{:>width$},{:>width$},{:>width$}]",
        usage.0,
        usage.1,
        usage.2,
        width = LEDGER_FIELD_WIDTH
    )
}

/// Counts the session's usage from disk. The ledger takes one directory
/// entry; `ledger_present` says whether the scan below already sees it.
fn initial_usage(
    directory: &Path,
    ledger_present: bool,
) -> Result<(usize, usize, usize), AppError> {
    let mut bytes = 0_usize;
    let mut entries = usize::from(!ledger_present);
    for entry in std::fs::read_dir(directory)? {
        entries += 1;
        if entries > WRITER_ENTRY_LIMIT {
            return Err(AppError::invalid_input_with_suggestion(
                "Session trace has too many directory entries to continue recording",
                "Start a new session.",
            ));
        }
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .and_then(crate::trace::trace_read::segment::parse_segment_filename)
            .is_some()
        {
            bytes = bytes
                .saturating_add(usize::try_from(entry.metadata()?.len()).unwrap_or(usize::MAX));
        }
    }
    let trace = crate::trace::trace_read::read_merged(
        directory,
        &crate::trace::trace_read::ReadOptions::default(),
    )?;
    if trace.warnings.iter().any(|warning| {
        matches!(
            warning.kind,
            crate::trace::trace_read::TraceWarningKind::UnreadableSegment
                | crate::trace::trace_read::TraceWarningKind::SymlinkedSegment
        )
    }) {
        return Err(AppError::invalid_input_with_suggestion(
            "Cannot initialize recording limits from an unreadable session trace",
            "Start a new session.",
        ));
    }
    Ok((bytes, trace.total_events, entries))
}

#[cfg(test)]
#[path = "trace_write_budget_tests.rs"]
mod tests;
