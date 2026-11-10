use agent_desktop_core::ProcessId;
use agent_desktop_core::system::launch_notes::LaunchNotes;
use agent_desktop_core::system::launch_result::LaunchResult;

use crate::system::process::process_facts;

/// The result for a spawned process that is still running but did not join
/// the accessibility bus while the launch waited for it.
pub(super) fn report_unlisted(id: &str, pid: u32) -> LaunchResult {
    let facts = process_facts::read(pid);
    LaunchResult {
        app: facts
            .as_ref()
            .map_or_else(|| id.to_owned(), crate::system::listed_app_name),
        pid: ProcessId::new(pid),
        process_instance: facts.map(|facts| facts.instance),
        window: None,
        cdp: None,
        renderer: None,
        notes: LaunchNotes {
            suggestion: Some(
                "The process is running but did not join the accessibility bus while the launch waited for it. Its pid is the spawned process's and may be a wrapper of the application. Its toolkit may need a flag or an environment variable to register; run 'list-apps' to see whether it appears.".to_owned(),
            ),
            launch_path: None,
        },
    }
}
