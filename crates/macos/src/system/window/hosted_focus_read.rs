//! Reads the focus and window evidence that `hosted_focus` decides on, and
//! keeps the proof needed to deliver keyboard input to a hosted view's owner.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};

use super::hosted_focus::{FocusPids, HostingEvidence, KeyboardRoute};
use crate::system::process::process_identity::ProcessIdentity;
use crate::tree::AXElement;

pub(crate) struct HostedFocus {
    host: i32,
    route: KeyboardRoute,
    host_focus: AXElement,
    owner: Option<ProcessIdentity>,
}

impl HostedFocus {
    pub(crate) fn is_hosted_view(&self) -> bool {
        self.owner.is_some()
    }

    pub(crate) fn details(&self) -> Option<serde_json::Value> {
        super::hosted_focus::route_details(self.host, self.route)
    }

    /// The pid that receives the key event: the hosted view's owner while the
    /// host still reports the same focused element and the owner is the same
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
    let mut pids = FocusPids {
        host,
        host_focus: element_pid(&host_focus, deadline),
        ..FocusPids::default()
    };
    if pids.host_focus == Some(host) {
        read_system_focus(&host_focus, &mut pids, deadline)?;
    }
    let Some(candidate) = super::hosted_focus::focus_owner_candidate(&pids) else {
        return Ok(None);
    };
    pids.focused_application = focused_application_pid(deadline);
    let focus_frame = frame(&host_focus, deadline)?;
    let windows = window_evidence(host, candidate)?;
    let route = super::hosted_focus::decide_route(
        &pids,
        &HostingEvidence {
            focus_frame,
            host_windows: &windows.host_windows,
            candidate_has_window: windows.candidate_has_window,
        },
    );
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
    tracing::debug!(?pids, ?focus_frame, ?route, "keyboard: hosted focus route");
    Ok(Some(HostedFocus {
        host,
        route,
        host_focus,
        owner,
    }))
}

fn read_system_focus(
    host_focus: &AXElement,
    pids: &mut FocusPids,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let Some(system_focus) = system_attribute("AXFocusedUIElement", deadline) else {
        return Ok(());
    };
    pids.system_focus = element_pid(&system_focus, deadline);
    if pids.system_focus == Some(pids.host) {
        return Ok(());
    }
    pids.system_focus_is_host_focus = crate::tree::same_element(&system_focus, host_focus)
        || frame(&system_focus, deadline)?
            .zip(frame(host_focus, deadline)?)
            .is_some_and(|(system, host)| super::hosted_focus::same_frame(&system, &host));
    Ok(())
}

fn host_focus(app: &AXElement, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let focused = crate::tree::surface_read::element(app, "AXFocusedUIElement", instant);
    deadline.ensure_remaining()?;
    Ok(focused.ok().flatten())
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

struct WindowEvidence {
    host_windows: Vec<Rect>,
    candidate_has_window: bool,
}

fn window_evidence(host: i32, candidate: i32) -> Result<WindowEvidence, AdapterError> {
    use super::cg_window::{WindowRecordScope, records_from_dictionaries, window_dictionaries};

    let dictionaries = window_dictionaries()?;
    let candidate_has_window = dictionaries.iter().any(|dictionary| {
        super::cg_window::int_field(dictionary, "kCGWindowOwnerPID") == Some(i64::from(candidate))
    });
    let host_windows = records_from_dictionaries(dictionaries, WindowRecordScope::Pid(host))?
        .into_iter()
        .filter(|record| record.visible)
        .map(|record| record.bounds)
        .collect();
    Ok(WindowEvidence {
        host_windows,
        candidate_has_window,
    })
}
