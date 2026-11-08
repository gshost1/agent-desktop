use agent_desktop_core::{AdapterError, Deadline, StateFile};
use serde::{Deserialize, Serialize};

use super::registry_entry::RegistryEntry;

const MAX_ENTRIES: usize = 32;
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryDocument {
    #[serde(default)]
    entries: Vec<RegistryEntry>,
}

/// The registry file under the state root: `web/endpoints.json`.
pub fn registry_file() -> Result<StateFile, AdapterError> {
    StateFile::under_state_root(&["web", "endpoints.json"])
}

/// Every entry in the registry. A missing, unreadable or malformed file
/// holds none: the registry is a hint, and ownership is proven before use.
pub fn read_entries(file: &StateFile) -> Vec<RegistryEntry> {
    file.read(MAX_BYTES)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice::<RegistryDocument>(&bytes).ok())
        .map(|document| document.entries)
        .unwrap_or_default()
}

/// Adds `entry`, replacing any entry for the same process or port, and keeps
/// the newest 32.
pub fn record_entry(
    file: &StateFile,
    entry: RegistryEntry,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let _lock = file.lock(deadline, "web endpoint registry")?;
    let mut entries = read_entries(file);
    entries.retain(|kept| {
        kept.port != entry.port
            && !(kept.pid == entry.pid && kept.process_instance == entry.process_instance)
    });
    entries.push(entry);
    let excess = entries.len().saturating_sub(MAX_ENTRIES);
    entries.drain(..excess);
    let bytes = serde_json::to_vec_pretty(&RegistryDocument { entries })
        .map_err(|error| AdapterError::internal(error.to_string()))?;
    file.write(&bytes)
}

#[cfg(test)]
#[path = "endpoint_registry_tests.rs"]
mod tests;
