use agent_desktop_core::{AdapterError, Deadline, RendererKind, WindowInfo};

use crate::system::SharedBus;
use crate::system::process::web_family::WebFamily;
use crate::system::registry::registry_frames::RootRead;

/// `Chromium` when the registry reads `pid` as a Chromium-family toolkit.
pub(super) fn chromium_renderer(
    bus: &SharedBus,
    pid: u32,
    deadline: Deadline,
) -> Option<RendererKind> {
    crate::system::registry::read_registry_for(bus, Some(pid), deadline)
        .ok()?
        .apps
        .iter()
        .any(|app| {
            crate::tree::properties::toolkit_trust::web_family(&app.toolkit)
                == Some(WebFamily::Chromium)
        })
        .then_some(RendererKind::Chromium)
}

/// Refreshes the same process-owned window after successful focus when a complete read is available.
pub(super) fn same_window(
    observation: Result<(Vec<WindowInfo>, RootRead), AdapterError>,
    expected: &WindowInfo,
) -> Option<WindowInfo> {
    let (windows, read) = observation.ok()?;
    if read != RootRead::Read {
        return None;
    }
    windows.into_iter().find(|window| {
        window.id == expected.id
            && window.pid == expected.pid
            && window.process_instance == expected.process_instance
    })
}
