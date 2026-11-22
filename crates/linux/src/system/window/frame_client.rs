use agent_desktop_core::Deadline;

use crate::system::process::toolkit_id::ToolkitId;
use crate::system::registry::registry_frames::{RootRead, read_frames};
use crate::system::window::frame_identity::permits_size_match;
use crate::system::x11::map::{ClientMatch, match_frames};
use crate::system::x11::{FrameFacts, X11Client};
use crate::system::{REGISTRY_ROOT_PATH, SharedBus};
use crate::tree::surfaces::top_level::is_top_level;
use crate::tree::{AtspiElement, RawNode};

fn match_frame(
    frame: &AtspiElement,
    nodes: &[RawNode],
    toolkit: &ToolkitId,
    clients: &[X11Client],
) -> ClientMatch {
    let windows: Vec<_> = nodes
        .iter()
        .filter(|node| is_top_level(&node.traits.interfaces))
        .collect();
    let Some(index) = windows.iter().position(|node| node.element == *frame) else {
        return ClientMatch::NoClient;
    };
    let frames: Vec<_> = windows
        .iter()
        .map(|node| {
            let extents = node.extents.present().copied();
            FrameFacts {
                size_identity: permits_size_match(node, nodes, toolkit, extents),
                pid: node.element.pid(),
                title: node.properties.name.present().cloned().unwrap_or_default(),
                extents,
                global_position: false,
            }
        })
        .collect();
    match_frames(&frames, clients)
        .get(index)
        .cloned()
        .unwrap_or(ClientMatch::NoClient)
}

/// Matches a local-position frame only after reading its complete sibling
/// inventory, so size identity and competing client claims are checked together.
pub(crate) fn read_client(
    bus: &SharedBus,
    frame: &AtspiElement,
    toolkit: &ToolkitId,
    clients: &[X11Client],
    deadline: Deadline,
) -> ClientMatch {
    let root = AtspiElement::new(frame.bus_name(), REGISTRY_ROOT_PATH, frame.pid());
    let Ok(mut apps) = read_frames(bus, &[root], deadline) else {
        return ClientMatch::NoClient;
    };
    let Some(app) = apps.pop().filter(|app| app.read == RootRead::Read) else {
        return ClientMatch::NoClient;
    };
    match_frame(frame, &app.nodes, toolkit, clients)
}

#[cfg(test)]
#[path = "frame_client_tests.rs"]
mod tests;
