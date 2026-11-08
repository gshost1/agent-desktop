use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_desktop_core::{
    AdapterError, Deadline, DownloadInfo, PlatformAdapter, ProcessIdentity, StateFile,
};

use super::download_event::{apply_event, is_download_event};
use super::download_failure::{canceled, interrupted, no_endpoint, timed_out, untracked};
use super::download_ledger::update_records;
use super::download_placement::{DiskState, disk_state, guid_path};
use super::download_record::DownloadRecord;
use super::download_settle::{ledger_deadline, place_finished};
use super::download_state::DownloadState;
use crate::channel::browser_link::BrowserLink;
use crate::channel::link_access::LinkAccess;
use crate::channel::web_channel_adapter::WebChannelAdapter;
use crate::transport::transport_error::is_timeout;

const POLL: Duration = Duration::from_millis(250);
const MISSING_GRACE: Duration = Duration::from_millis(1_500);

enum Step {
    Ready(DownloadInfo),
    Failed(AdapterError),
    Pending(DownloadRecord),
    Missing,
    Idle,
}

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Returns the browser's oldest download no earlier wait returned,
    /// waiting for it to finish or for a new one to begin. The ledger holds
    /// what earlier invocations saw; this connection's own events and, with
    /// a download folder, the folder itself say how each one ends.
    pub(crate) fn wait_web_download(
        &self,
        process: &ProcessIdentity,
        deadline: Deadline,
    ) -> Result<DownloadInfo, AdapterError> {
        let ledger = self.download_ledger()?;
        self.with_link(process, LinkAccess::prompting(deadline), |link| {
            wait_on_link(link, ledger, deadline)
        })?
        .ok_or_else(no_endpoint)
    }
}

fn wait_on_link(
    link: &mut BrowserLink,
    ledger: &StateFile,
    deadline: Deadline,
) -> Result<DownloadInfo, AdapterError> {
    let mut arrived = Vec::new();
    let mut missing_since: Option<Instant> = None;
    loop {
        arrived.extend(link.connection.drain_events(is_download_event));
        link.note_seen(&arrived);
        let events = std::mem::take(&mut arrived);
        let dir = link.download_dir.clone();
        let grace_over = missing_since.is_some_and(|since| since.elapsed() >= MISSING_GRACE);
        let step = update_records(ledger, &link.process, ledger_deadline()?, |records| {
            for event in &events {
                apply_event(records, event);
            }
            next_step(records, dir.as_deref(), &link.seen_downloads, grace_over)
        })?;
        let pending = match step {
            Step::Ready(info) => return Ok(info),
            Step::Failed(error) => return Err(error),
            Step::Missing => {
                missing_since.get_or_insert_with(Instant::now);
                None
            }
            Step::Pending(record) => {
                missing_since = None;
                Some(record)
            }
            Step::Idle => {
                missing_since = None;
                None
            }
        };
        if deadline.is_expired() {
            return Err(timed_out(pending.as_ref()));
        }
        match link
            .connection
            .wait_for_event(is_download_event, deadline.capped(POLL))
        {
            Ok(event) => arrived.push(event),
            Err(error) if is_timeout(&error) => {}
            Err(error) => return Err(error),
        }
    }
}

/// Settles the oldest download no wait returned yet. With a download folder
/// an in-progress download is read from the folder, since its events may
/// have gone to a connection that closed; a partial file that vanished
/// without a finished one is interrupted once `grace_over`. Without a
/// folder only this connection's events can finish it.
fn next_step(
    records: &mut [DownloadRecord],
    dir: Option<&Path>,
    seen: &HashSet<String>,
    grace_over: bool,
) -> Step {
    let Some(record) = records.iter_mut().find(|record| !record.delivered) else {
        return Step::Idle;
    };
    if record.state == DownloadState::InProgress {
        match dir {
            Some(dir) => match disk_state(dir, &record.guid) {
                DiskState::Finished(bytes) => {
                    record.state = DownloadState::Completed;
                    record.bytes = bytes;
                    record.path = guid_path(dir, &record.guid).map(|path| path_text(&path));
                }
                DiskState::Partial => return Step::Pending(record.clone()),
                DiskState::Missing if grace_over => record.state = DownloadState::Interrupted,
                DiskState::Missing => return Step::Missing,
            },
            None if seen.contains(&record.guid) => return Step::Pending(record.clone()),
            None => {
                record.delivered = true;
                return Step::Failed(untracked(record));
            }
        }
    }
    record.delivered = true;
    match record.state {
        DownloadState::Completed => {
            place_finished(record, dir);
            match record.path.clone() {
                Some(path) => Step::Ready(DownloadInfo {
                    path,
                    suggested_filename: record.suggested_filename.clone(),
                    url: record.url.clone(),
                    state: record.state.as_str().to_owned(),
                    bytes: record.bytes,
                }),
                None => Step::Failed(interrupted(record)),
            }
        }
        DownloadState::Canceled => Step::Failed(canceled(record)),
        DownloadState::Interrupted | DownloadState::InProgress => Step::Failed(interrupted(record)),
    }
}

pub(crate) fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
#[path = "download_wait_tests.rs"]
mod tests;
