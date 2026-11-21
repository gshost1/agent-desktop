use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;

use super::child_target::ChildTarget;
use super::child_targets::{attach_child_targets, stop_child_targets};
use super::flatten::flatten;
use super::frame_budget::FrameBudget;
use super::frame_graft::{FrameNodes, graft_frames};
use super::target_job::TargetJob;
use super::target_read::{TargetReads, read_targets};
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
    auto_attached.reverse();
    stop_child_targets(connection, &auto_attached, budget.snapshot());
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

/// Reads the page target, then the out-of-process iframe targets one level
/// of nesting at a time: every parent of a level is asked for its children
/// at once, and every child of the level is read at once. Frames and
/// unreadable frames are listed in the order a breadth-first read of one
/// target after another gives, which the grafting and the depth-first ref
/// order rely on.
fn read_page(
    connection: &mut CdpConnection,
    session: &PageSession,
    budget: FrameBudget,
    auto_attached: &mut Vec<PageSession>,
) -> Result<(Vec<FrameNodes>, Vec<String>), AdapterError> {
    let page = TargetJob {
        session,
        reads: TargetReads {
            budget,
            is_page: true,
        },
        parent: None,
        parent_frame_id: None,
    };
    let (mut frames, mut unreadable) = read_targets(connection, &[page])?
        .pop()
        .unwrap_or_else(|| Err(no_tree(&session.target_id)))?;
    let mut parents = vec![session.clone()];
    while !parents.is_empty() {
        let level = attach_level(connection, session, budget, parents, auto_attached)?;
        let mut jobs = Vec::with_capacity(level.len());
        let mut asked = Vec::with_capacity(level.len());
        for (parent, child) in &level {
            let read = !budget.is_spent();
            if read {
                jobs.push(TargetJob {
                    session: &child.session,
                    reads: TargetReads {
                        budget,
                        is_page: false,
                    },
                    parent: Some(parent),
                    parent_frame_id: child.parent_frame_id.as_deref(),
                });
            }
            asked.push(read);
        }
        let mut reads = read_targets(connection, &jobs)?.into_iter();
        parents = Vec::new();
        for ((_, child), read) in level.iter().zip(asked) {
            let read = if read { reads.next() } else { None };
            match read {
                None => unreadable.push(child.session.target_id.clone()),
                Some(Ok((child_frames, child_unreadable))) => {
                    frames.extend(child_frames);
                    unreadable.extend(child_unreadable);
                    parents.push(child.session.clone());
                }
                Some(Err(error))
                    if is_protocol_error(&error)
                        || is_target_gone(&error)
                        || budget.missed(&error) =>
                {
                    unreadable.push(child.session.target_id.clone());
                }
                Some(Err(error)) => return Err(error),
            }
        }
    }
    Ok((frames, unreadable))
}

fn attach_level(
    connection: &mut CdpConnection,
    page: &PageSession,
    budget: FrameBudget,
    parents: Vec<PageSession>,
    auto_attached: &mut Vec<PageSession>,
) -> Result<Vec<(PageSession, ChildTarget)>, AdapterError> {
    let asked: Vec<(PageSession, Deadline)> = parents
        .into_iter()
        .filter_map(|parent| {
            let is_page = parent == *page;
            if !is_page && budget.is_spent() {
                return None;
            }
            let deadline = if is_page {
                budget.snapshot()
            } else {
                budget.iframes()
            };
            Some((parent, deadline))
        })
        .collect();
    let attached = attach_child_targets(connection, &asked)?;
    let mut level = Vec::new();
    for ((parent, _), children) in asked.into_iter().zip(attached) {
        let is_page = parent == *page;
        match children {
            Ok(children) => {
                auto_attached.push(parent.clone());
                level.extend(children.into_iter().map(|child| (parent.clone(), child)));
            }
            Err(error) if !is_page && is_target_gone(&error) => {}
            Err(error) if !is_page && budget.missed(&error) => auto_attached.push(parent),
            Err(error) => return Err(error),
        }
    }
    Ok(level)
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
