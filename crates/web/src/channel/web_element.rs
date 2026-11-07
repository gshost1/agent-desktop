use agent_desktop_core::actions::capability;
use agent_desktop_core::tree::state;
use agent_desktop_core::{
    AccessibilityNode, AdapterError, Deadline, ElementState, IdentifierEvidence, LiveElement,
    LiveIdentity, LocatorField, NativeHandle, PlatformAdapter, RefEntry,
};

use super::web_channel_adapter::WebChannelAdapter;
use super::web_handle::WebHandle;
use crate::{ElementReading, ElementToken, read_element, read_node};

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Re-identifies a ref captured over CDP: reconnects to its browser,
    /// re-attaches its page and resolves its backend node. A node that is
    /// gone, or whose role or name changed since capture, is `STALE_REF`.
    pub(crate) fn resolve_web_ref(
        &self,
        entry: &RefEntry,
        token: &str,
        deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let token = ElementToken::parse(token)?;
        let process = entry.process.identity().ok_or_else(|| {
            AdapterError::stale_ref_because("The web ref does not name its browser process")
        })?;
        let node = self.with_required_link(&process, deadline, |link| {
            let session = link.session(&token.target_id, deadline)?;
            read_node(
                &mut link.connection,
                &session,
                token.backend_node_id,
                deadline,
            )
        })?;
        if node.role != entry.identity.role || node.identity.name != entry.identity.name {
            return Err(AdapterError::stale_ref_because(
                "The web element changed since the snapshot",
            )
            .with_suggestion("Take a new snapshot and use the ref it returns")
            .with_details(serde_json::json!({
                "kind": "cdp_node_drift",
                "captured": { "role": entry.identity.role, "name": entry.identity.name },
                "live": { "role": node.role, "name": node.identity.name },
            })));
        }
        Ok(NativeHandle::new(WebHandle { process, token }))
    }

    /// A fresh read of a web element: its accessibility node and its DOM
    /// value and checked state, which win over the accessibility values.
    pub(crate) fn web_live_element(
        &self,
        handle: &WebHandle,
        deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        let token = &handle.token;
        let (node, reading) = self.with_required_link(&handle.process, deadline, |link| {
            let session = link.session(&token.target_id, deadline)?;
            let node = read_node(
                &mut link.connection,
                &session,
                token.backend_node_id,
                deadline,
            )?;
            let reading = read_element(
                &mut link.connection,
                &session,
                token.backend_node_id,
                deadline,
            )?;
            Ok((node, reading))
        })?;
        Ok(live_element(node, reading, token))
    }
}

/// Builds the live element of one page node. A page node has no screen
/// bounds: it is reported as on-screen and nonspatial, so actions on it are
/// delivered through the page, never through the pointer.
pub fn live_element(
    node: AccessibilityNode,
    reading: ElementReading,
    token: &ElementToken,
) -> LiveElement {
    let mut states = node.presentation.states;
    if let Some(checked) = reading.checked {
        states.retain(|name| name != state::CHECKED);
        if checked {
            states.push(state::CHECKED.into());
        }
    }
    let mut available_actions = node.presentation.available_actions;
    for action in [capability::PRESS_KEY, capability::SCROLL_TO] {
        if !available_actions.iter().any(|known| known == action) {
            available_actions.push(action.into());
        }
    }
    let identity = node.identity;
    let identifiers = IdentifierEvidence::typed(
        identity.native_id.clone(),
        identity.native_id.as_ref().map(|_| 0),
        true,
    )
    .with_retained_object(token.to_string());
    LiveElement {
        identity: LiveIdentity {
            name: field(identity.name),
            description: field(identity.description),
            identifiers,
        },
        state: ElementState {
            role: node.role,
            value: reading.value.or(identity.value),
            enabled: Some(!state::has_state(&states, state::DISABLED)),
            hidden: Some(state::has_state(&states, state::HIDDEN)),
            offscreen: Some(false),
            states,
        },
        states_complete: true,
        bounds: None,
        available_actions,
    }
}

fn field(value: Option<String>) -> LocatorField<String> {
    value.map_or(LocatorField::Absent, LocatorField::Known)
}

#[cfg(test)]
#[path = "web_element_tests.rs"]
mod tests;
