use std::path::Path;

use agent_desktop_core::{AdapterError, Deadline, PlatformAdapter, ProcessIdentity};

use super::action_settle::settle_action;
use super::download_event::{apply_event, is_download_event, is_download_start};
use super::download_ledger::{update_downloads, update_records};
use super::download_placement::{guid_path, place};
use super::download_record::DownloadRecord;
use super::download_state::DownloadState;
use super::download_wait::path_text;
use super::folder_listing::FolderListing;
use crate::channel::browser_link::BrowserLink;
use crate::channel::web_channel_adapter::WebChannelAdapter;

const LEDGER_LOCK_MS: u64 = 500;

/// The bound on taking the download ledger's lock. It is detached from the
/// command's deadline so a wait whose time ran out still records what it saw.
pub(crate) fn ledger_deadline() -> Result<Deadline, AdapterError> {
    Deadline::detached_after(LEDGER_LOCK_MS)
}

/// Gives a finished guid-named download its suggested name. A download
/// whose path is already elsewhere, or a folder that refuses the move,
/// keeps the path it has.
pub(crate) fn place_finished(record: &mut DownloadRecord, dir: Option<&Path>) {
    let Some(dir) = dir else {
        return;
    };
    if record.state != DownloadState::Completed {
        return;
    }
    let guid_named = guid_path(dir, &record.guid).map(|path| path_text(&path));
    if record.path.is_some() && record.path != guid_named {
        return;
    }
    match place(dir, &record.guid, &record.suggested_filename) {
        Ok(placed) => record.path = Some(path_text(&placed)),
        Err(error) => {
            tracing::warn!(error = %error, guid = %record.guid, "web download kept its guid name");
            record.path = record.path.take().or(guid_named);
        }
    }
}

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Records the download events this connection holds in the ledger, so
    /// a later `wait --download` sees them. With `settle`, it first waits for
    /// what the action set off to settle ([`settle_action`]), so a download
    /// the action starts begins while this connection still points it at the
    /// download folder. A failure here never fails the action. Returns
    /// whether a download began.
    pub(crate) fn note_downloads(
        &self,
        link: &mut BrowserLink,
        settle: bool,
        deadline: Deadline,
    ) -> bool {
        let mut events = Vec::new();
        if settle {
            let started = std::time::Instant::now();
            let settled = settle_action(&mut link.connection, deadline);
            tracing::debug!(
                elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                began = settled.as_ref().is_ok_and(|starts| !starts.is_empty()),
                "web action settle"
            );
            events.extend(settled.unwrap_or_default());
        }
        events.extend(link.connection.drain_events(is_download_event));
        if events.is_empty() {
            return false;
        }
        link.note_seen(&events);
        let began = events.iter().any(is_download_start);
        let Ok(ledger) = self.download_ledger() else {
            return began;
        };
        let dir = link.guid_folder().map(Path::to_path_buf);
        let recorded = ledger_deadline().and_then(|lock_deadline| {
            update_records(ledger, &link.process, lock_deadline, |records| {
                for event in &events {
                    apply_event(records, event);
                }
                for record in records.iter_mut() {
                    place_finished(record, dir.as_deref());
                }
            })
        });
        if let Err(error) = recorded {
            tracing::warn!(error = %error, "web downloads were not recorded");
        }
        began
    }

    /// Marks every file already in a profile download folder as not a
    /// download, so `wait --download` returns only what the browser saves
    /// after launch. A failure only risks returning an older file.
    pub(crate) fn claim_existing_downloads(&self, process: &ProcessIdentity, dir: &Path) {
        let Ok(ledger) = self.download_ledger() else {
            return;
        };
        let listing = FolderListing::read(dir);
        let claimed = ledger_deadline().and_then(|lock_deadline| {
            update_downloads(ledger, process, lock_deadline, |entry| {
                for file in &listing.finished {
                    entry.claim(&file.name);
                }
            })
        });
        if let Err(error) = claimed {
            tracing::warn!(error = %error, "web downloads: files already in the folder were not recorded");
        }
    }
}
