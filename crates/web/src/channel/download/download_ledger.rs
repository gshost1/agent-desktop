use agent_desktop_core::{AdapterError, Deadline, ProcessIdentity, StateFile};
use serde::{Deserialize, Serialize};

use super::download_record::DownloadRecord;
use super::process_downloads::ProcessDownloads;

const MAX_PROCESSES: usize = 16;
const MAX_RECORDS: usize = 32;
const MAX_BYTES: u64 = 256 * 1024;

#[derive(Debug, Default, Serialize, Deserialize)]
struct LedgerDocument {
    #[serde(default)]
    processes: Vec<ProcessDownloads>,
}

/// The download ledger under the state root: `web/downloads.json`. Each
/// invocation that sees a browser's download events adds them here, so a
/// later `wait --download` can return a download that began or finished
/// before it ran.
pub(crate) fn ledger_file() -> Result<StateFile, AdapterError> {
    StateFile::under_state_root(&["web", "downloads.json"])
}

/// Runs `edit` on the records of `process` under the ledger's lock and
/// writes the ledger back when `edit` changed it. The newest 16 browser
/// processes are kept, and for each its newest 32 downloads, dropping
/// delivered ones first.
pub(crate) fn update_records<T>(
    file: &StateFile,
    process: &ProcessIdentity,
    deadline: Deadline,
    edit: impl FnOnce(&mut Vec<DownloadRecord>) -> T,
) -> Result<T, AdapterError> {
    let _lock = file.lock(deadline, "web download ledger")?;
    let mut document = read_document(file);
    let pid = u32::from(process.pid);
    let index = match document
        .processes
        .iter()
        .position(|entry| entry.pid == pid && entry.process_instance == process.instance)
    {
        Some(index) => index,
        None => {
            document.processes.push(ProcessDownloads {
                pid,
                process_instance: process.instance.clone(),
                records: Vec::new(),
            });
            document.processes.len() - 1
        }
    };
    let before = document.processes[index].records.clone();
    let outcome = edit(&mut document.processes[index].records);
    if document.processes[index].records == before {
        return Ok(outcome);
    }
    prune(&mut document.processes[index].records);
    let entry = document.processes.remove(index);
    document.processes.push(entry);
    let excess = document.processes.len().saturating_sub(MAX_PROCESSES);
    document.processes.drain(..excess);
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| AdapterError::internal(error.to_string()))?;
    file.write(&bytes)?;
    Ok(outcome)
}

fn read_document(file: &StateFile) -> LedgerDocument {
    file.read(MAX_BYTES)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<LedgerDocument>(&bytes).ok())
        .unwrap_or_default()
}

fn prune(records: &mut Vec<DownloadRecord>) {
    while records.len() > MAX_RECORDS {
        let oldest = records
            .iter()
            .position(|record| record.delivered)
            .unwrap_or(0);
        records.remove(oldest);
    }
}

#[cfg(test)]
#[path = "download_ledger_tests.rs"]
mod tests;
