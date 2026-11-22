use agent_desktop_core::ProcessId;
use agent_desktop_core::system::launch_notes::LaunchNotes;
use agent_desktop_core::system::launch_result::LaunchResult;

use super::launch_owner_query::off_bus::{self, OffBus};
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
    let title = (seen == OffBus::Windowed)
        .then(|| off_bus::window_title(pid))
        .flatten();
    LaunchResult {
        notes: LaunchNotes {
            suggestion: Some(suggestion(&app, seen, title.as_deref())),
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

fn suggestion(app: &str, seen: OffBus, title: Option<&str>) -> String {
    let wait = match title.filter(|title| !title.trim().is_empty()) {
        Some(title) => format!(
            "agent-desktop wait --app {} --window {} --timeout 30000",
            shell_quoted(app),
            shell_quoted(title)
        ),
        None => format!(
            "agent-desktop wait --event window-opened --app {} --timeout 30000",
            shell_quoted(app)
        ),
    };
    match seen {
        OffBus::Starting => format!(
            "The application is still starting: its process is running, but it has neither joined the accessibility bus nor shown a window within the launch timeout. To wait for its window, run: {wait}. {WRAPPER}"
        ),
        OffBus::Windowed => format!(
            "The process shows a window but did not join the accessibility bus within the launch timeout. {WRAPPER} Its toolkit may need a flag or an environment variable to register (for example, Qt may need QT_ACCESSIBILITY=1). To wait for its accessible window, run: {wait}."
        ),
        OffBus::BridgeOff => format!(
            "The process runs with its accessibility bridge turned off (NO_AT_BRIDGE=1 or GTK_A11Y=none in its environment), so it does not join the accessibility bus. {WRAPPER} Launch it without that setting to read or drive it."
        ),
    }
}

/// `text` as one POSIX shell word.
fn shell_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "launch_unlisted_tests.rs"]
mod tests;
