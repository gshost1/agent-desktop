use std::path::Path;
use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, PlatformAdapter};

use super::download_event::{apply_event, is_download_event, is_download_start};
use super::download_ledger::update_records;
use super::download_placement::{guid_path, place};
use super::download_record::DownloadRecord;
use super::download_state::DownloadState;
use super::download_wait::path_text;
use crate::channel::browser_link::BrowserLink;
use crate::channel::web_channel_adapter::WebChannelAdapter;

const DOWNLOAD_NOTICE: Duration = Duration::from_millis(200);
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
    /// a later `wait --download` sees them. After an action that can start a
    /// download on a browser with a download folder, it first waits up to
    /// 200 ms for one to begin: the browser drops the download folder when
    /// this connection closes, so a download it has not yet begun would land
    /// in the browser's own folder. A failure here never fails the action.
    /// Returns whether a download began.
    pub(crate) fn note_downloads(
        &self,
        link: &mut BrowserLink,
        may_start: bool,
        deadline: Deadline,
    ) -> bool {
        let mut events = Vec::new();
        let buffered_start = link.connection.drain_events(is_download_start);
        let lingers = may_start && link.download_dir.is_some() && buffered_start.is_empty();
        events.extend(buffered_start);
        if lingers
            && let Ok(event) = link
                .connection
                .wait_for_event(is_download_start, deadline.capped(DOWNLOAD_NOTICE))
        {
            events.push(event);
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
        let dir = link.download_dir.clone();
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
}
