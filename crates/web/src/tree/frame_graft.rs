use std::collections::HashSet;

use super::web_node::WebNode;

/// One frame's built nodes, ready to be grafted under its owner element.
pub(crate) struct FrameNodes {
    pub(crate) frame_id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) owner_backend_node_id: Option<i64>,
    pub(crate) nodes: Vec<WebNode>,
}

/// Grafts every child frame's nodes beneath the element that owns the frame
/// in its parent frame, deepest frames first, and returns the main frame's
/// nodes with the ids of the frames that could not be placed. Frames come
/// in an order where each parent precedes its children. A frame whose owner
/// element is not in the tree is appended to its parent frame's first root.
/// A frame whose parent frame is absent, because that frame was unreadable
/// or was itself dropped, is dropped and reported.
pub(crate) fn graft_frames(frames: Vec<FrameNodes>) -> (Vec<WebNode>, Vec<String>) {
    let (mut frames, dropped) = split_unreachable(frames);
    while frames.len() > 1 {
        let Some(child) = frames.pop() else {
            break;
        };
        let Some(parent) = frames
            .iter_mut()
            .find(|frame| Some(&frame.frame_id) == child.parent_id.as_ref())
        else {
            continue;
        };
        let owner = child
            .owner_backend_node_id
            .and_then(|backend| find_owner(&mut parent.nodes, &parent.frame_id, backend));
        match owner {
            Some(owner) => owner.children.extend(child.nodes),
            None => {
                if let Some(first) = parent.nodes.first_mut() {
                    first.children.extend(child.nodes);
                }
            }
        }
    }
    let main = frames.pop().map(|frame| frame.nodes).unwrap_or_default();
    (main, dropped)
}

fn split_unreachable(frames: Vec<FrameNodes>) -> (Vec<FrameNodes>, Vec<String>) {
    let mut placed: HashSet<String> = HashSet::new();
    let mut kept = Vec::with_capacity(frames.len());
    let mut dropped = Vec::new();
    for frame in frames {
        let reachable = match frame.parent_id.as_ref() {
            _ if kept.is_empty() => true,
            Some(parent) => placed.contains(parent),
            None => false,
        };
        if reachable {
            placed.insert(frame.frame_id.clone());
            kept.push(frame);
        } else {
            dropped.push(frame.frame_id);
        }
    }
    (kept, dropped)
}

fn find_owner<'a>(
    nodes: &'a mut [WebNode],
    frame_id: &str,
    backend: i64,
) -> Option<&'a mut WebNode> {
    for node in nodes {
        let is_owner = node
            .origin
            .as_ref()
            .is_some_and(|origin| origin.backend_node_id == backend && origin.frame_id == frame_id);
        if is_owner {
            return Some(node);
        }
        if let Some(found) = find_owner(&mut node.children, frame_id, backend) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
#[path = "frame_graft_tests.rs"]
mod tests;
