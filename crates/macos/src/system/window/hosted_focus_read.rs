//! Reads the focus, process and window evidence that `hosted_focus` decides
//! on, and keeps the proof needed to deliver keyboard input to the owner.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};
use std::time::Duration;

use super::hosted_focus::Candidate;
use super::keyboard_route::KeyboardRoute;
use crate::system::process::process_identity::ProcessIdentity;
use crate::tree::AXElement;

const MAX_FRAME_CHECKS: usize = 8;
const OWNER_READ_LIMIT: Duration = Duration::from_millis(250);

pub(crate) struct HostedFocus {
    host: i32,
    route: KeyboardRoute,
    candidates: Vec<Candidate>,
    host_focus: AXElement,
    owner: Option<ProcessIdentity>,
}

impl HostedFocus {
    pub(crate) fn is_hosted_view(&self) -> bool {
        self.owner.is_some()
    }

    pub(crate) fn details(&self) -> Option<serde_json::Value> {
        self.route.details(self.host, &self.candidates)
    }

    /// The pid that receives the key event: the focus owner while the host
    /// still reports the same focused element and the owner is the same
    /// process instance, otherwise the host.
    pub(crate) fn delivery_pid(
        &self,
        app: &AXElement,
        deadline: Deadline,
    ) -> Result<i32, AdapterError> {
        let Some(owner) = self.owner else {
            return Ok(self.host);
        };
        let still_focused = host_focus(app, deadline)?
            .is_some_and(|focused| crate::tree::same_element(&focused, &self.host_focus));
        if still_focused && owner.still_matches()? {
            return Ok(owner.pid());
        }
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Keyboard focus left the hosted view before key delivery",
        )
        .with_details(serde_json::json!({
            "host_pid": self.host,
            "focus_owner_pid": owner.pid(),
            "physical_delivery_started": false,
        }))
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion(
            "Read the panel again with 'list-surfaces --app <app>', then retry the key",
        ))
    }
}

pub(crate) fn resolve(
    app: &AXElement,
    host: i32,
    deadline: Deadline,
) -> Result<Option<HostedFocus>, AdapterError> {
    let Some(host_focus) = host_focus(app, deadline)? else {
        return Ok(None);
    };
    let mut candidates = gather_candidates(app, &host_focus, host, deadline)?;
    tracing::debug!(
        host,
        ?candidates,
        "keyboard: hosted focus candidates gathered"
    );
    if candidates.is_empty() {
        return Ok(None);
    }
    let windows = super::hosted_focus_process::window_owners(host)?;
    let host_frame = frame(&host_focus, deadline)?;
    let mut frame_checks = 0;
    for candidate in &mut candidates {
        candidate.owns_window = windows.owners.contains(&candidate.pid);
        candidate.responsible_to_host |=
            super::hosted_focus_process::responsible_pid(candidate.pid) == Some(host);
        let receivable = candidate.owns_window || candidate.focused_application;
        if candidate.owns_host_focus || !receivable || frame_checks >= MAX_FRAME_CHECKS {
            continue;
        }
        frame_checks += 1;
        candidate.focus_frame_matches = match host_frame {
            Some(host_frame) => Some(
                owner_focus_frame(candidate.pid, deadline)?
                    .is_some_and(|owner| super::hosted_focus::same_frame(&owner, &host_frame)),
            ),
            None => None,
        };
    }
    let host_focus_has_window = needs_window_read(&candidates)
        .then(|| has_window(&host_focus, deadline))
        .transpose()?
        .unwrap_or(true);
    candidates.retain(|candidate| candidate.is_relevant(host_focus_has_window));
    candidates.sort_by_key(|candidate| {
        (
            !candidate.owns_host_focus,
            candidate.focus_frame_matches != Some(true),
            !candidate.focused_application,
            !candidate.responsible_to_host,
        )
    });
    let route = super::hosted_focus::decide_route(
        super::hosted_focus::frame_inside(host_frame, &windows.host_windows),
        &candidates,
    );
    finish(host, route, candidates, host_focus)
}

fn finish(
    host: i32,
    route: KeyboardRoute,
    candidates: Vec<Candidate>,
    host_focus: AXElement,
) -> Result<Option<HostedFocus>, AdapterError> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let owner = match route.hosted_pid() {
        Some(pid) => ProcessIdentity::capture(pid)?,
        None => None,
    };
    let route = match (route, owner) {
        (KeyboardRoute::HostedView { pid }, None) => KeyboardRoute::Unproven {
            pid,
            reason: "owner_process_exited",
        },
        (route, _) => route,
    };
    tracing::debug!(?candidates, ?route, "keyboard: hosted focus route");
    Ok(Some(HostedFocus {
        host,
        route,
        candidates,
        host_focus,
        owner,
    }))
}

fn gather_candidates(
    app: &AXElement,
    host_focus: &AXElement,
    host: i32,
    deadline: Deadline,
) -> Result<Vec<Candidate>, AdapterError> {
    let mut candidates = Vec::new();
    if let Some(pid) = element_pid(host_focus, deadline).filter(|pid| *pid != host) {
        entry(&mut candidates, pid).owns_host_focus = true;
    }
    if is_frontmost(app, deadline)?
        && let Some(pid) = focused_application_pid(deadline).filter(|pid| *pid != host)
    {
        let candidate = entry(&mut candidates, pid);
        candidate.focused_application = true;
        if system_focus_is_host_focus(host_focus, pid, deadline)? {
            entry(&mut candidates, pid).owns_host_focus = true;
        }
    }
    for child in super::hosted_focus_process::responsible_children(host) {
        entry(&mut candidates, child).responsible_to_host = true;
    }
    Ok(candidates)
}

fn entry(candidates: &mut Vec<Candidate>, pid: i32) -> &mut Candidate {
    let index = match candidates.iter().position(|candidate| candidate.pid == pid) {
        Some(index) => index,
        None => {
            candidates.push(Candidate {
                pid,
                ..Candidate::default()
            });
            candidates.len() - 1
        }
    };
    &mut candidates[index]
}

fn needs_window_read(candidates: &[Candidate]) -> bool {
    candidates.iter().any(|candidate| {
        candidate.responsible_to_host
            && !candidate.owns_host_focus
            && !candidate.focused_application
            && candidate.focus_frame_matches != Some(true)
    })
}

fn system_focus_is_host_focus(
    host_focus: &AXElement,
    pid: i32,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let Some(system_focus) = system_attribute("AXFocusedUIElement", deadline) else {
        return Ok(false);
    };
    if element_pid(&system_focus, deadline) != Some(pid) {
        return Ok(false);
    }
    if crate::tree::same_element(&system_focus, host_focus) {
        return Ok(true);
    }
    Ok(frame(&system_focus, deadline)?
        .zip(frame(host_focus, deadline)?)
        .is_some_and(|(system, host)| super::hosted_focus::same_frame(&system, &host)))
}

fn owner_focus_frame(pid: i32, deadline: Deadline) -> Result<Option<Rect>, AdapterError> {
    let owner_deadline = deadline.capped(OWNER_READ_LIMIT);
    let owner = crate::tree::element_for_pid(pid);
    let focused = host_focus(&owner, owner_deadline);
    deadline.ensure_remaining()?;
    match focused.ok().flatten() {
        Some(focused) => {
            let read = frame(&focused, owner_deadline);
            deadline.ensure_remaining()?;
            Ok(read.ok().flatten())
        }
        None => Ok(None),
    }
}

fn host_focus(app: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let focused = crate::tree::surface_read::element(app, "AXFocusedUIElement", instant);
    deadline.ensure_remaining()?;
    Ok(focused.ok().flatten())
}

fn has_window(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let window = crate::tree::surface_read::element(element, "AXWindow", instant);
    deadline.ensure_remaining()?;
    Ok(window.ok().flatten().is_some())
}

fn is_frontmost(app: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(app, deadline)?;
    let frontmost =
        crate::tree::properties::attributes::copy_bool_attr_result(app, "AXFrontmost", deadline);
    deadline.ensure_remaining()?;
    Ok(frontmost.ok().flatten().unwrap_or(false))
}

fn system_attribute(attribute: &str, deadline: Deadline) -> Option<AXElement> {
    let system =
        crate::tree::AXElement(unsafe { accessibility_sys::AXUIElementCreateSystemWide() });
    let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    crate::tree::surface_read::element(&system, attribute, instant)
        .ok()
        .flatten()
}

fn focused_application_pid(deadline: Deadline) -> Option<i32> {
    system_attribute("AXFocusedApplication", deadline)
        .and_then(|application| element_pid(&application, deadline))
}

fn element_pid(element: &AXElement, deadline: Deadline) -> Option<i32> {
    crate::tree::properties::ax_ipc::pid(element, deadline).ok()
}

fn frame(element: &AXElement, deadline: Deadline) -> Result<Option<Rect>, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let frame = crate::tree::element_bounds::read_bounds_with_deadline(element, instant);
    deadline.ensure_remaining()?;
    Ok(frame.ok().flatten())
}
