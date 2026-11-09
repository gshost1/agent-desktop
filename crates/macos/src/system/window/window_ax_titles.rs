//! Window titles proven through accessibility.
//!
//! CoreGraphics leaves `kCGWindowName` empty unless the caller holds Screen
//! Recording permission, so a window's title can need the accessibility read.
//! A title is joined to a CoreGraphics window only through the
//! `_AXUIElementGetWindow` bridge of an AX window of the same process: the
//! bridge reports the exact CoreGraphics window number, which is the identity
//! proof the window resolver already accepts. Geometry and title similarity
//! never join a title, because two windows can share both.

use rustc_hash::FxHashMap;

use crate::system::window::cg_window::WindowRecord;

/// Titles keyed by the CoreGraphics window number an AX window bridged to.
/// A `None` entry marks a bridged window whose title is not provable: the AX
/// window reported no title, or two AX windows bridged to the same number with
/// different titles.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WindowAxTitles(FxHashMap<i64, Option<String>>);

impl WindowAxTitles {
    pub(crate) fn record(&mut self, window_number: i64, title: Option<String>) {
        let title = title.filter(|title| !title.is_empty());
        match self.0.entry(window_number) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(title);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if *entry.get() != title {
                    entry.insert(None);
                }
            }
        }
    }

    pub(crate) fn contains(&self, window_number: i64) -> bool {
        self.0.contains_key(&window_number)
    }

    pub(crate) fn title_for(&self, window_number: i64) -> Option<&str> {
        self.0.get(&window_number).and_then(Option::as_deref)
    }

    /// Gives a record without a CoreGraphics name the title of the AX window
    /// that bridged to it. The caller passes the titles read from the record's
    /// own process. A CoreGraphics name, when present, is kept.
    pub(crate) fn fill_missing(&self, record: &mut WindowRecord) {
        if record.title.is_some() {
            return;
        }
        record.title = self.title_for(record.window_number).map(str::to_owned);
    }
}

#[cfg(test)]
#[path = "window_ax_titles_tests.rs"]
mod tests;
