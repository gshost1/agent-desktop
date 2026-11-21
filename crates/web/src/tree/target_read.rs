use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::cdp_ax_node::CdpAxNode;
use super::dom_facts::DomFacts;
use super::dom_ids::{capture_params, dom_facts_from_capture};
use super::frame_budget::FrameBudget;
use super::frame_build::build_frame;
use super::frame_context::FrameContext;
use super::frame_entry::FrameEntry;
use super::frame_graft::FrameNodes;
use super::frame_tree::frames_from_reply;
use super::target_job::TargetJob;
use crate::cdp::cdp_request::CdpRequest;
use crate::cdp::protocol_error::{is_protocol_error, is_target_gone};
use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession};

/// The frames one target rendered, in pre-order, and the frames it could not
/// read; or the error that lost the whole target.
pub(crate) type TargetRead = Result<(Vec<FrameNodes>, Vec<String>), AdapterError>;

type Reply = Option<Result<Value, AdapterError>>;

type FramePlan = Vec<Option<(usize, Option<usize>)>>;

/// How one target's frames are read: the page's own target reads its main
/// frame within the snapshot deadline and its in-process iframes within the
/// iframe budget; an iframe target reads all of its frames within that budget.
#[derive(Clone, Copy)]
pub(crate) struct TargetReads {
    pub(crate) budget: FrameBudget,
    pub(crate) is_page: bool,
}

impl TargetReads {
    pub(crate) fn target_deadline(self) -> Deadline {
        if self.is_page {
            self.budget.snapshot()
        } else {
            self.budget.iframes()
        }
    }
}

/// Builds the nodes of every frame each job's target renders, in pre-order,
/// and lists the frames whose tree the target refused to read. All targets
/// are read together in two pipelined rounds, one listing their frames and
/// DOM and one reading each frame's tree and owner element, so the browser
/// answers different targets concurrently; each job's outcome is what
/// reading its target alone would give. A page's main frame must be
/// readable; any other frame that is not is skipped. When an iframe target's
/// own root frame is unreadable, every frame of that target is listed and
/// none is built.
pub(crate) fn read_targets(
    connection: &mut CdpConnection,
    jobs: &[TargetJob<'_>],
) -> Result<Vec<TargetRead>, AdapterError> {
    let listing: Vec<CdpRequest> = jobs
        .iter()
        .flat_map(|job| {
            let deadline = job.reads.target_deadline();
            [
                request("Page.getFrameTree", json!({}), job.session, deadline),
                request(
                    "DOMSnapshot.captureSnapshot",
                    capture_params(),
                    job.session,
                    deadline,
                ),
            ]
        })
        .collect();
    let mut listed = connection.call_batch(&listing)?.into_iter();
    let mut targets = Vec::with_capacity(jobs.len());
    for _ in jobs {
        let (Some(frames), Some(capture)) = (listed.next(), listed.next()) else {
            return Err(reply_missing());
        };
        targets.push(listed_target(frames, capture));
    }
    let mut requests = Vec::new();
    let plans: Vec<Option<(FramePlan, Option<usize>)>> = jobs
        .iter()
        .zip(&targets)
        .map(|(job, target)| {
            let (frames, _) = target.as_ref().ok()?;
            Some(plan_frames(job, frames, &mut requests))
        })
        .collect();
    let mut replies: Vec<Reply> = connection
        .call_batch(&requests)?
        .into_iter()
        .map(Some)
        .collect();
    Ok(jobs
        .iter()
        .zip(targets)
        .zip(plans)
        .map(|((job, target), plan)| {
            let (frames, dom) = target?;
            let (plan, root_owner) = plan.ok_or_else(reply_missing)?;
            let built = build_frames(job, &frames, &dom, plan, &mut replies)?;
            attach_root_owner(job, built, root_owner, &mut replies)
        })
        .collect())
}

fn listed_target(
    frames: Result<Value, AdapterError>,
    capture: Result<Value, AdapterError>,
) -> Result<(Vec<FrameEntry>, DomFacts), AdapterError> {
    let frames = frames_from_reply(&frames?)?;
    let dom = match capture {
        Ok(capture) => dom_facts_from_capture(&capture),
        Err(error) if is_protocol_error(&error) => DomFacts::default(),
        Err(error) => return Err(error),
    };
    Ok((frames, dom))
}

fn plan_frames(
    job: &TargetJob<'_>,
    frames: &[FrameEntry],
    requests: &mut Vec<CdpRequest>,
) -> (FramePlan, Option<usize>) {
    let mut push = |request: CdpRequest| {
        requests.push(request);
        requests.len() - 1
    };
    let reads = job.reads;
    let plan = frames
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let in_page_iframe = reads.is_page && index > 0;
            if in_page_iframe && reads.budget.is_spent() {
                return None;
            }
            let deadline = if in_page_iframe {
                reads.budget.iframes()
            } else {
                reads.target_deadline()
            };
            let frame = json!({ "frameId": entry.frame_id });
            let tree = push(request(
                "Accessibility.getFullAXTree",
                frame.clone(),
                job.session,
                deadline,
            ));
            let owner = (index > 0)
                .then(|| push(request("DOM.getFrameOwner", frame, job.session, deadline)));
            Some((tree, owner))
        })
        .collect();
    let root_owner = job.parent.zip(frames.first()).map(|(parent, root)| {
        let frame = json!({ "frameId": root.frame_id });
        push(request(
            "DOM.getFrameOwner",
            frame,
            parent,
            reads.budget.iframes(),
        ))
    });
    (plan, root_owner)
}

fn build_frames(
    job: &TargetJob<'_>,
    frames: &[FrameEntry],
    dom: &DomFacts,
    plan: FramePlan,
    replies: &mut [Reply],
) -> TargetRead {
    let reads = job.reads;
    let mut built = Vec::with_capacity(frames.len());
    let mut unreadable = Vec::new();
    for ((index, entry), step) in frames.iter().enumerate().zip(plan) {
        let in_page_iframe = reads.is_page && index > 0;
        let Some((tree, owner)) = step else {
            unreadable.push(entry.frame_id.clone());
            continue;
        };
        let nodes = match take(replies, tree) {
            Ok(reply) => parse_nodes(reply)?,
            Err(error) if entry.parent_id.is_some() && is_protocol_error(&error) => {
                if index == 0 {
                    let all = frames.iter().map(|frame| frame.frame_id.clone()).collect();
                    return Ok((Vec::new(), all));
                }
                unreadable.push(entry.frame_id.clone());
                continue;
            }
            Err(error) if in_page_iframe && reads.budget.missed(&error) => {
                unreadable.push(entry.frame_id.clone());
                continue;
            }
            Err(error) => return Err(error),
        };
        let owner_backend_node_id = match owner.map(|owner| owner_from_reply(take(replies, owner)))
        {
            None => None,
            Some(Ok(owner)) => owner,
            Some(Err(error)) if in_page_iframe && reads.budget.missed(&error) => {
                unreadable.push(entry.frame_id.clone());
                continue;
            }
            Some(Err(error)) => return Err(error),
        };
        let context = FrameContext {
            target_id: &job.session.target_id,
            frame_id: &entry.frame_id,
            loader_id: &entry.loader_id,
            dom,
        };
        built.push(FrameNodes {
            frame_id: entry.frame_id.clone(),
            parent_id: entry.parent_id.clone(),
            owner_backend_node_id,
            nodes: build_frame(nodes, &context),
        });
    }
    Ok((built, unreadable))
}

fn attach_root_owner(
    job: &TargetJob<'_>,
    (mut frames, unreadable): (Vec<FrameNodes>, Vec<String>),
    root_owner: Option<usize>,
    replies: &mut [Reply],
) -> TargetRead {
    if let (Some(root), Some(owner)) = (frames.first_mut(), root_owner) {
        if root.parent_id.is_none() {
            root.parent_id = job.parent_frame_id.map(str::to_owned);
        }
        root.owner_backend_node_id = owner_from_reply(take(replies, owner))?;
    }
    Ok((frames, unreadable))
}

fn owner_from_reply(reply: Result<Value, AdapterError>) -> Result<Option<i64>, AdapterError> {
    match reply {
        Ok(reply) => Ok(reply.get("backendNodeId").and_then(Value::as_i64)),
        Err(error) if is_protocol_error(&error) || is_target_gone(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn request(
    method: &'static str,
    params: Value,
    session: &PageSession,
    deadline: Deadline,
) -> CdpRequest {
    CdpRequest {
        method,
        params,
        session_id: Some(session.session_id.clone()),
        deadline,
    }
}

fn take(replies: &mut [Reply], index: usize) -> Result<Value, AdapterError> {
    replies
        .get_mut(index)
        .and_then(Option::take)
        .unwrap_or_else(|| Err(reply_missing()))
}

fn reply_missing() -> AdapterError {
    transport_failed("snapshot", "a pipelined reply is missing")
}

fn parse_nodes(mut reply: Value) -> Result<Vec<CdpAxNode>, AdapterError> {
    let nodes = reply.get_mut("nodes").map_or(Value::Null, Value::take);
    serde_json::from_value(nodes)
        .map_err(|error| transport_failed("Accessibility.getFullAXTree", error.to_string()))
}

#[cfg(test)]
#[path = "target_read_tests.rs"]
mod tests;
