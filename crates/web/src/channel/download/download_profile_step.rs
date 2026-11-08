use std::collections::HashSet;
use std::path::Path;

use agent_desktop_core::DownloadInfo;

use super::download_failure::{canceled, interrupted};
use super::download_record::DownloadRecord;
use super::download_state::DownloadState;
use super::download_step::DownloadStep;
use super::download_wait::path_text;
use super::folder_file::FolderFile;
use super::folder_listing::{FolderListing, saved_as};
use super::process_downloads::ProcessDownloads;

/// Settles the oldest download no wait returned yet for a browser whose
/// profile saves into `dir` under real file names. A download whose start a
/// connection recorded is matched to its file by the name it suggested; one
/// no connection saw is the oldest finished file no wait returned. The
/// browser writes a `.crdownload` file until a download finishes, so a
/// recorded download with neither file is interrupted once `grace_over`.
pub(crate) fn next_profile_step(
    entry: &mut ProcessDownloads,
    dir: &Path,
    seen: &HashSet<String>,
    grace_over: bool,
) -> DownloadStep {
    let listing = FolderListing::read(dir);
    let unclaimed: Vec<FolderFile> = listing
        .finished
        .iter()
        .filter(|file| !entry.is_claimed(&file.name))
        .cloned()
        .collect();
    let Some(index) = entry.records.iter().position(|record| !record.delivered) else {
        return unrecorded(entry, dir, &listing, unclaimed.first());
    };
    let following = seen.contains(&entry.records[index].guid);
    let record = &mut entry.records[index];
    let unsettled = record.state == DownloadState::InProgress
        || (record.state == DownloadState::Completed && record.path.is_none());
    if unsettled {
        let named = unclaimed
            .iter()
            .find(|file| saved_as(&record.suggested_filename, &file.name));
        let renamed = (grace_over && listing.partial.is_empty())
            .then(|| unclaimed.first())
            .flatten();
        match named.or(renamed) {
            Some(file) => {
                record.state = DownloadState::Completed;
                record.bytes = file.bytes;
                record.path = Some(path_text(&dir.join(&file.name)));
            }
            None if following || !listing.partial.is_empty() => {
                return DownloadStep::Pending(record.clone());
            }
            None if grace_over => record.state = DownloadState::Interrupted,
            None => return DownloadStep::Missing,
        }
    }
    record.delivered = true;
    let record = record.clone();
    settle(entry, &record)
}

fn settle(entry: &mut ProcessDownloads, record: &DownloadRecord) -> DownloadStep {
    match (record.state, record.path.as_deref()) {
        (DownloadState::Completed, Some(path)) => {
            if let Some(name) = Path::new(path).file_name().and_then(|name| name.to_str()) {
                entry.claim(name);
            }
            DownloadStep::Ready(DownloadInfo {
                path: path.to_owned(),
                suggested_filename: record.suggested_filename.clone(),
                url: Some(record.url.clone()),
                state: record.state.as_str().to_owned(),
                bytes: record.bytes,
            })
        }
        (DownloadState::Canceled, _) => DownloadStep::Failed(canceled(record)),
        _ => DownloadStep::Failed(interrupted(record)),
    }
}

/// A download no connection saw begin: the oldest finished file no wait
/// returned, or a partial file still being written.
fn unrecorded(
    entry: &mut ProcessDownloads,
    dir: &Path,
    listing: &FolderListing,
    oldest: Option<&FolderFile>,
) -> DownloadStep {
    if let Some(file) = oldest {
        entry.claim(&file.name);
        return DownloadStep::Ready(DownloadInfo {
            path: path_text(&dir.join(&file.name)),
            suggested_filename: file.name.clone(),
            url: None,
            state: DownloadState::Completed.as_str().to_owned(),
            bytes: file.bytes,
        });
    }
    match listing.partial.first() {
        Some(name) => DownloadStep::Pending(DownloadRecord {
            guid: String::new(),
            url: String::new(),
            suggested_filename: name.clone(),
            state: DownloadState::InProgress,
            bytes: 0,
            path: None,
            delivered: false,
        }),
        None => DownloadStep::Idle,
    }
}

#[cfg(test)]
#[path = "download_profile_step_tests.rs"]
mod tests;
