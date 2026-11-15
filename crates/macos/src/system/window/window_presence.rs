//! Where a CoreGraphics window sits: off screen, on screen at the normal
//! window level, or on screen at a floating level.
//!
//! Floating levels, `kCGFloatingWindowLevel` (3) up to `kCGUtilityWindowLevel`
//! (19), below the Dock, hold an application's utility panels: font and colour
//! panels, inspectors, palettes. The same levels also carry helper windows that
//! nothing can act on, so a floating window is listed only while it is on
//! screen and its own process publishes it as an accessibility window, proven
//! through the `_AXUIElementGetWindow` bridge that already joins titles.

use agent_desktop_core::AdapterError;
use rustc_hash::FxHashMap;

use crate::system::window::cg_window::WindowRecord;
use crate::system::window::window_ax_titles::WindowAxTitles;

const NORMAL_LEVEL: i64 = 0;
const FLOATING_LEVELS: std::ops::RangeInclusive<i64> = 1..=19;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum WindowPresence {
    Offscreen,
    Onscreen,
    Floating,
}

impl WindowPresence {
    /// The presence of a window at `layer`, or `None` for a level the
    /// inventory never lists. `onscreen` is read only for a level that can be
    /// listed, so a malformed record at another level is skipped, not fatal.
    pub(crate) fn classify(
        layer: i64,
        onscreen: impl FnOnce() -> Result<bool, AdapterError>,
    ) -> Result<Option<Self>, AdapterError> {
        if layer == NORMAL_LEVEL {
            return Ok(Some(Self::from_onscreen(onscreen()?)));
        }
        if FLOATING_LEVELS.contains(&layer) && onscreen()? {
            return Ok(Some(Self::Floating));
        }
        Ok(None)
    }

    pub(crate) fn from_onscreen(onscreen: bool) -> Self {
        if onscreen {
            Self::Onscreen
        } else {
            Self::Offscreen
        }
    }

    pub(crate) fn is_onscreen(self) -> bool {
        self != Self::Offscreen
    }

    pub(crate) fn is_floating(self) -> bool {
        self == Self::Floating
    }
}

/// Drops every floating record its process does not publish as an
/// accessibility window. Each process with a floating record is read once; a
/// process that cannot be read loses its floating records and keeps the rest.
pub(crate) fn retain_published_floating(
    records: &mut Vec<WindowRecord>,
    mut published: impl FnMut(i32) -> Result<WindowAxTitles, AdapterError>,
) {
    let mut by_pid: FxHashMap<i32, Option<WindowAxTitles>> = FxHashMap::default();
    records.retain(|record| {
        if !record.presence.is_floating() {
            return true;
        }
        by_pid
            .entry(record.pid)
            .or_insert_with(|| published(record.pid).ok())
            .as_ref()
            .is_some_and(|titles| titles.contains(record.window_number))
    });
}

/// Reads which windows `pid` publishes through accessibility, within a short
/// probe budget so a busy process cannot stall an inventory.
pub(crate) fn published_windows(
    pid: i32,
    deadline: std::time::Instant,
) -> Result<WindowAxTitles, AdapterError> {
    let probe = std::time::Instant::now()
        .checked_add(std::time::Duration::from_millis(250))
        .map_or(deadline, |probe| probe.min(deadline));
    crate::system::window::window_ax_state::accessible_window_titles_until(
        pid,
        &rustc_hash::FxHashSet::default(),
        probe,
    )
}

#[cfg(test)]
#[path = "window_presence_tests.rs"]
mod tests;
