use std::borrow::Cow;

use agent_desktop_core::{ProcessIdentity, RefEntry, SnapshotSurface, WindowInfo};

use crate::ElementToken;

/// The element token of a ref captured over CDP, if it is one.
pub fn web_token(entry: &RefEntry) -> Option<&str> {
    entry
        .identity
        .retained_object
        .as_deref()
        .filter(|token| ElementToken::is_cdp_token(token))
}

/// `entry` as the platform adapter must see it. A ref read through the
/// accessibility tree of a browser window on the web surface is a window
/// ref to the platform adapter, which has never heard of the web surface.
pub fn platform_entry(entry: &RefEntry) -> Cow<'_, RefEntry> {
    if entry.source.source_surface != SnapshotSurface::Web {
        return Cow::Borrowed(entry);
    }
    let mut translated = entry.clone();
    translated.source.source_surface = SnapshotSurface::Window;
    Cow::Owned(translated)
}

/// The process that owns `window`, when the platform reported its instance.
pub fn window_process(window: &WindowInfo) -> Option<ProcessIdentity> {
    window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .map(|instance| ProcessIdentity::new(window.pid, instance))
}

#[cfg(test)]
#[path = "ref_routing_tests.rs"]
mod tests;
