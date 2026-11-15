use agent_desktop_core::ProcessId;
use agent_desktop_core::system::launch_notes::LaunchNotes;
use agent_desktop_core::system::launch_result::LaunchResult;

use super::launch_owner_query::off_bus::OffBus;
use crate::system::process::process_facts;

const WRAPPER: &str = "Its pid is the spawned process's and may be a wrapper of the application.";

/// The result for a spawned process that is still running but did not join
/// the accessibility bus while the launch waited for it, with the next step
/// that what the launch saw of the process calls for.
pub(super) fn report_unlisted(id: &str, pid: u32, seen: OffBus) -> LaunchResult {
    let facts = process_facts::read(pid);
    let app = facts
        .as_ref()
        .map_or_else(|| id.to_owned(), crate::system::listed_app_name);
    LaunchResult {
        notes: LaunchNotes {
            suggestion: Some(suggestion(&app, seen)),
            launch_path: None,
        },
        app,
        pid: ProcessId::new(pid),
        process_instance: facts.map(|facts| facts.instance),
        window: None,
        cdp: None,
        renderer: None,
    }
}

fn suggestion(app: &str, seen: OffBus) -> String {
    match seen {
        OffBus::Starting => format!(
            "The application is still starting: its process is running, but it has neither joined the accessibility bus nor shown a window within the launch timeout. To wait for its window, run: agent-desktop wait --app '{app}' --window '<title>' --timeout 30000. Run 'agent-desktop list-apps' to see when it appears. {WRAPPER}"
        ),
        OffBus::Windowed => format!(
            "The process shows a window but did not join the accessibility bus. {WRAPPER} Its toolkit may need a flag or an environment variable to register; run 'list-apps' to see whether it appears."
        ),
        OffBus::BridgeOff => format!(
            "The process runs with its accessibility bridge turned off (NO_AT_BRIDGE=1 or GTK_A11Y=none in its environment), so it does not join the accessibility bus. {WRAPPER} Launch it without that setting to read or drive it."
        ),
    }
}

#[cfg(test)]
#[path = "launch_unlisted_tests.rs"]
mod tests;
