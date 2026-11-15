//! Names the sheets, popovers and alerts of every window of an application
//! from the window server's window number, the identity `list-windows`
//! reports as `w-<number>`. A surface keeps its id while focus moves between
//! windows and while sibling elements come and go, so a baseline diff reports
//! only surfaces that really opened or closed.

use agent_desktop_core::SurfaceInfo;
use std::collections::HashSet;

/// One accessibility window of the application and the surfaces read from it.
pub(crate) struct WindowSurfaces {
    pub(crate) number: Option<i64>,
    pub(crate) own: Option<SurfaceRead>,
    pub(crate) children: Vec<SurfaceRead>,
}

/// A sheet, popover or alert element, with its own window number when the
/// window server gives it one distinct from its parent's.
pub(crate) struct SurfaceRead {
    pub(crate) number: Option<i64>,
    pub(crate) kind: &'static str,
    pub(crate) title: Option<String>,
}

pub(crate) const UNREAD_WINDOW_OWNER_ID: &str = "app/windows";

const WINDOW_SURFACE_KINDS: [&str; 3] = ["sheet", "popover", "alert"];

pub(crate) fn surfaces_from_windows(windows: Vec<WindowSurfaces>) -> Vec<SurfaceInfo> {
    let mut seen = HashSet::new();
    let mut surfaces = Vec::new();
    let mut unread = false;
    for window in windows {
        let Some(number) = window.number else {
            unread |= window.own.is_some() || !window.children.is_empty();
            continue;
        };
        let window_id = format!("w-{number}");
        if let Some(own) = window.own {
            push_unique(&mut seen, &mut surfaces, window_id.clone(), own);
        }
        let mut ordinals: Vec<(&'static str, usize)> = Vec::new();
        for child in window.children {
            let id = match child.number.filter(|own| *own != number) {
                Some(own) => format!("w-{own}"),
                None => format!(
                    "{window_id}/{}/{}",
                    child.kind,
                    next_ordinal(&mut ordinals, child.kind)
                ),
            };
            push_unique(&mut seen, &mut surfaces, id, child);
        }
    }
    if unread {
        surfaces.push(unread_window_surfaces());
    }
    surfaces
}

fn next_ordinal(ordinals: &mut Vec<(&'static str, usize)>, kind: &'static str) -> usize {
    match ordinals.iter_mut().find(|(seen, _)| *seen == kind) {
        Some((_, count)) => {
            *count += 1;
            *count
        }
        None => {
            ordinals.push((kind, 0));
            0
        }
    }
}

fn push_unique(
    seen: &mut HashSet<String>,
    surfaces: &mut Vec<SurfaceInfo>,
    id: String,
    read: SurfaceRead,
) {
    if seen.insert(id.clone()) {
        surfaces.push(SurfaceInfo {
            id,
            kind: read.kind.into(),
            title: read.title,
            item_count: None,
            unclassified: Vec::new(),
        });
    }
}

/// A window the window server gives no number has no stable identity, so its
/// surfaces are reported as unread rather than under an id that changes.
fn unread_window_surfaces() -> SurfaceInfo {
    SurfaceInfo {
        id: UNREAD_WINDOW_OWNER_ID.into(),
        kind: "window".into(),
        title: None,
        item_count: None,
        unclassified: WINDOW_SURFACE_KINDS
            .iter()
            .map(|kind| (*kind).into())
            .collect(),
    }
}

#[cfg(test)]
#[path = "surface_window_ids_tests.rs"]
mod tests;
