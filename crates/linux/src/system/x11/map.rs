use super::{FrameFacts, X11Client, X11Evidence, X11State};
use crate::tree::Extents;

/// Which X11 clients a listed window could be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClientMatch {
    One(u32),
    Several(Vec<u32>),
    NoClient,
}

fn frame_around(client: &X11Client) -> Option<Extents> {
    let bounds = client.bounds?;
    let edges = client.extents.unwrap_or_default();
    let left = i32::try_from(edges.left).ok()?;
    let right = i32::try_from(edges.right).ok()?;
    let top = i32::try_from(edges.top).ok()?;
    let bottom = i32::try_from(edges.bottom).ok()?;
    Some(Extents {
        x: bounds.x.checked_sub(left)?,
        y: bounds.y.checked_sub(top)?,
        width: bounds.width.checked_add(left)?.checked_add(right)?,
        height: bounds.height.checked_add(top)?.checked_add(bottom)?,
    })
}

fn without_shadow(client: &X11Client, frame: Extents) -> Extents {
    let (left, top, right, bottom) = client.shadow;
    Extents {
        x: frame.x + left,
        y: frame.y + top,
        width: frame.width - left - right,
        height: frame.height - top - bottom,
    }
}

fn sits_at(client: &X11Client, extents: Extents) -> bool {
    frame_around(client)
        .is_some_and(|frame| frame == extents || without_shadow(client, frame) == extents)
}

fn same_size(left: Extents, right: Extents) -> bool {
    left.width == right.width && left.height == right.height
}

pub(crate) fn exact_geometry_matches(frame: &FrameFacts, client: &X11Client) -> bool {
    frame.global_position
        && frame.extents.is_some_and(|size| {
            size.width > 0
                && size.height > 0
                && size.x != i32::MIN
                && size.y != i32::MIN
                && (sits_at(client, size) || client.bounds == Some(size))
        })
}

pub(crate) fn size_matches(frame: &FrameFacts, client: &X11Client) -> bool {
    let Some(size) = frame
        .extents
        .filter(|size| size.width > 0 && size.height > 0)
    else {
        return false;
    };
    frame_around(client).is_some_and(|outer| {
        same_size(size, outer) || same_size(size, without_shadow(client, outer))
    }) || client.bounds.is_some_and(|bounds| same_size(size, bounds))
}

pub(crate) fn geometry_client(frame: &FrameFacts, clients: &[X11Client]) -> ClientMatch {
    if !frame.global_position && !frame.size_identity {
        return ClientMatch::NoClient;
    }
    let Some(_) = frame.extents.filter(|size| {
        size.width > 0 && size.height > 0 && size.x != i32::MIN && size.y != i32::MIN
    }) else {
        return ClientMatch::NoClient;
    };
    let matching: Vec<u32> = clients
        .iter()
        .filter(|client| client.pid == Some(frame.pid))
        .filter(|client| {
            if frame.global_position {
                exact_geometry_matches(frame, client)
            } else {
                size_matches(frame, client)
            }
        })
        .map(|client| client.id)
        .collect();
    match matching.as_slice() {
        [] => ClientMatch::NoClient,
        [only] => ClientMatch::One(*only),
        _ => ClientMatch::Several(matching),
    }
}

/// The client of a window, by process id and title first. A tie is broken by
/// global extents. Without a title match, exact same-process geometry identifies
/// the client: the client, frame or shadowless frame rectangle for global
/// toolkits, or their sizes for local ones. Several candidates stay ambiguous;
/// child order is never used. Local frames without a title match require the
/// caller's size-identity proof. Missing or invalid geometry identifies nothing.
pub(crate) fn match_client(frame: &FrameFacts, clients: &[X11Client]) -> ClientMatch {
    if frame.title.is_empty() && !frame.global_position {
        return if frame.size_identity {
            geometry_client(frame, clients)
        } else {
            ClientMatch::NoClient
        };
    }
    let named: Vec<&X11Client> = clients
        .iter()
        .filter(|client| client.pid == Some(frame.pid) && client.title == frame.title)
        .collect();
    let [_, _, ..] = named.as_slice() else {
        return named.first().map_or_else(
            || {
                if frame.global_position || frame.size_identity {
                    geometry_client(frame, clients)
                } else {
                    ClientMatch::NoClient
                }
            },
            |client| ClientMatch::One(client.id),
        );
    };
    if let (true, Some(extents)) = (frame.global_position, frame.extents) {
        let placed: Vec<&&X11Client> = named
            .iter()
            .filter(|client| sits_at(client, extents))
            .collect();
        if let [only] = placed.as_slice() {
            return ClientMatch::One(only.id);
        }
    }
    ClientMatch::Several(named.iter().map(|client| client.id).collect())
}

pub(crate) fn has_title_match(frame: &FrameFacts, clients: &[X11Client]) -> bool {
    (!frame.title.is_empty() || frame.global_position)
        && clients
            .iter()
            .any(|client| client.pid == Some(frame.pid) && client.title == frame.title)
}

/// Matches a coherent set of frames without letting geometry reclaim a named
/// client's identity or assigning one client to competing geometry-only frames.
pub(crate) fn match_frames(frames: &[FrameFacts], clients: &[X11Client]) -> Vec<ClientMatch> {
    let named: Vec<bool> = frames
        .iter()
        .map(|frame| has_title_match(frame, clients))
        .collect();
    let available: Vec<X11Client> = clients
        .iter()
        .filter(|client| {
            !frames.iter().zip(&named).any(|(frame, named)| {
                *named && Some(frame.pid) == client.pid && frame.title == client.title
            })
        })
        .cloned()
        .collect();
    let matches: Vec<ClientMatch> = frames
        .iter()
        .zip(&named)
        .map(|(frame, named)| match_client(frame, if *named { clients } else { &available }))
        .collect();
    matches
        .iter()
        .enumerate()
        .map(|(index, found)| {
            if !named[index]
                && let ClientMatch::One(id) = found
                && matches.iter().enumerate().any(|(other, found)| {
                    other != index
                        && match found {
                            ClientMatch::One(other) => other == id,
                            ClientMatch::Several(ids) => ids.contains(id),
                            ClientMatch::NoClient => false,
                        }
                })
            {
                return ClientMatch::NoClient;
            }
            found.clone()
        })
        .collect()
}

/// What the window manager's answer says about the listed windows.
pub(crate) fn evidence(state: &X11State, frames: &[FrameFacts]) -> X11Evidence {
    let X11State::Ready { active, clients } = state else {
        return X11Evidence::Unavailable;
    };
    let Some(active) = active else {
        return X11Evidence::NoActive;
    };
    let mut exact = Vec::new();
    let mut possible = false;
    for (index, found) in match_frames(frames, clients).into_iter().enumerate() {
        match found {
            ClientMatch::One(id) if id == *active => exact.push(index),
            ClientMatch::Several(ids) if ids.contains(active) => possible = true,
            _ => {}
        }
    }
    match (exact.as_slice(), possible) {
        ([only], false) => X11Evidence::Frame(*only),
        ([], false) => X11Evidence::Unmapped,
        _ => X11Evidence::Ambiguous,
    }
}

#[cfg(test)]
#[path = "map_tests.rs"]
mod tests;
