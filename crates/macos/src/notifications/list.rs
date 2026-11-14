use agent_desktop_core::{
    AdapterError, Deadline, InteractionPolicy, NotificationFilter, NotificationInfo,
};

use super::nc_session::{NcSession, close_session};

pub fn list_notifications(
    filter: &NotificationFilter,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Vec<NotificationInfo>, AdapterError> {
    let session = NcSession::open(policy, deadline)?;
    let result = list_from_nc(filter, session.pid(), deadline);
    close_session(session, result)
}

#[cfg(target_os = "macos")]
fn list_from_nc(
    filter: &NotificationFilter,
    pid: i32,
    deadline: Deadline,
) -> Result<Vec<NotificationInfo>, AdapterError> {
    let entries = list_entries(filter, pid, deadline)?;
    Ok(entries.into_iter().map(|e| e.info).collect())
}

#[cfg(target_os = "macos")]
pub(super) struct NotificationEntry {
    pub info: NotificationInfo,
    pub element: crate::tree::AXElement,
}

#[cfg(target_os = "macos")]
pub(super) fn list_entries(
    filter: &NotificationFilter,
    pid: i32,
    deadline: Deadline,
) -> Result<Vec<NotificationEntry>, AdapterError> {
    use crate::tree::element_for_pid;

    let app = element_for_pid(pid);
    let windows = crate::notifications::read::children_for_attribute(&app, "AXWindows", deadline)?;
    if windows.is_empty() {
        return Ok(vec![]);
    }

    let mut scan = super::scan::NotificationScan::new(filter, deadline);
    for window in &windows {
        let top_children = crate::notifications::read::children(window, deadline)?;
        scan.collect(&top_children, 0)?;
        if scan.is_full() {
            break;
        }
    }
    Ok(scan.finish())
}

#[cfg(not(target_os = "macos"))]
fn list_from_nc(
    _filter: &NotificationFilter,
    _pid: i32,
    _deadline: Deadline,
) -> Result<Vec<NotificationInfo>, AdapterError> {
    Err(AdapterError::not_supported("list_notifications"))
}
