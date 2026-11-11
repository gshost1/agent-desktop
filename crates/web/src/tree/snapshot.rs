use std::collections::VecDeque;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::child_target::ChildTarget;
use super::child_targets::{attach_child_targets, stop_child_targets};
use super::flatten::flatten;
use super::frame_budget::FrameBudget;
use super::frame_graft::{FrameNodes, graft_frames};
use super::target_read::{frame_owner, read_target};
use crate::cdp::protocol_error::{is_protocol_error, is_target_gone};
use crate::{CdpConnection, PageSession, WebSnapshot};

/// Reads the full accessibility tree of every frame in the page, including
/// out-of-process iframes, which are attached as their own targets, grafts
/// each child frame beneath its owner element, and returns it in core form.
/// Frames that cannot be read, and frames lost because an ancestor frame
/// could not be read, are listed in `unreadable_frames`. Every iframe is read
/// within `deadline`, and one that misses it fails the snapshot.
pub fn snapshot_page(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<WebSnapshot, AdapterError> {
    snapshot_with(connection, session, FrameBudget::whole(deadline))
}

/// Reads the page as [`snapshot_page`] does, but stops reading iframes a
/// reserve short of `deadline`, so a slow iframe cannot cost the page. An
/// iframe that misses that budget is listed in `unreadable_frames`, and the
/// child frames of an iframe reached after it are not looked for.
pub fn snapshot_page_within_budget(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<WebSnapshot, AdapterError> {
    snapshot_with(connection, session, FrameBudget::reserved(deadline))
}

fn snapshot_with(
    connection: &mut CdpConnection,
    session: &PageSession,
    budget: FrameBudget,
) -> Result<WebSnapshot, AdapterError> {
    let mut auto_attached = Vec::new();
    let read = read_page(connection, session, budget, &mut auto_attached);
    for parent in auto_attached.iter().rev() {
        stop_child_targets(connection, parent, budget.snapshot());
    }
    let (frames, mut unreadable_frames) = read?;
    let (roots, dropped) = graft_frames(frames);
    unreadable_frames.extend(dropped);
    let mut roots = roots.into_iter();
    let (Some(root), None) = (roots.next(), roots.next()) else {
        return Err(no_tree(&session.target_id));
    };
    let mut origins = Vec::new();
    let tree = flatten(root, &mut origins);
    Ok(WebSnapshot {
        tree,
        origins,
        unreadable_frames,
    })
}

fn read_page(
    connection: &mut CdpConnection,
    session: &PageSession,
    budget: FrameBudget,
    auto_attached: &mut Vec<PageSession>,
) -> Result<(Vec<FrameNodes>, Vec<String>), AdapterError> {
    let (mut frames, mut unreadable) = read_target(connection, session, budget.snapshot())?;
    let mut parents = VecDeque::from([session.clone()]);
    while let Some(parent) = parents.pop_front() {
        let is_page = parent == *session;
        if !is_page && budget.is_spent() {
            continue;
        }
        let attach_deadline = if is_page {
            budget.snapshot()
        } else {
            budget.iframes()
        };
        let children = match attach_child_targets(connection, &parent, attach_deadline) {
            Ok(children) => children,
            Err(error) if !is_page && is_target_gone(&error) => continue,
            Err(error) if !is_page && budget.missed(&error) => {
                auto_attached.push(parent.clone());
                continue;
            }
            Err(error) => return Err(error),
        };
        auto_attached.push(parent.clone());
        for child in children {
            if budget.is_spent() {
                unreadable.push(child.session.target_id.clone());
                continue;
            }
            match read_child(connection, &parent, &child, budget.iframes()) {
                Ok((child_frames, child_unreadable)) => {
                    frames.extend(child_frames);
                    unreadable.extend(child_unreadable);
                }
                Err(error)
                    if is_protocol_error(&error)
                        || is_target_gone(&error)
                        || budget.missed(&error) =>
                {
                    unreadable.push(child.session.target_id.clone());
                    continue;
                }
                Err(error) => return Err(error),
            }
            parents.push_back(child.session);
        }
    }
    Ok((frames, unreadable))
}

fn read_child(
    connection: &mut CdpConnection,
    parent: &PageSession,
    child: &ChildTarget,
    deadline: Deadline,
) -> Result<(Vec<FrameNodes>, Vec<String>), AdapterError> {
    let (mut frames, unreadable) = read_target(connection, &child.session, deadline)?;
    if let Some(root) = frames.first_mut() {
        if root.parent_id.is_none() {
            root.parent_id.clone_from(&child.parent_frame_id);
        }
        root.owner_backend_node_id = frame_owner(connection, parent, &root.frame_id, deadline)?;
    }
    Ok((frames, unreadable))
}

fn no_tree(target_id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Page target {target_id} exposed no single accessibility tree root"),
    )
    .with_details(json!({ "kind": "cdp_tree_empty", "target_id": target_id }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
