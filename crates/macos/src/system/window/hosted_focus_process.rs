//! Process and window-server evidence for `hosted_focus`: which processes
//! macOS holds the application responsible for, which processes own a window,
//! and where the application's visible windows are.

use agent_desktop_core::{AdapterError, Rect};
use rustc_hash::FxHashSet;
use std::ffi::c_void;
use std::sync::OnceLock;

type ResponsiblePid = unsafe extern "C" fn(libc::pid_t) -> libc::pid_t;

pub(crate) struct WindowOwners {
    pub(crate) host_windows: Vec<Rect>,
    pub(crate) owners: FxHashSet<i32>,
}

/// The process macOS attributes `pid`'s work to: an app's XPC services, such as
/// a panel it shows from another process, name that app. `None` when the
/// system does not expose the relation or the process is gone.
pub(crate) fn responsible_pid(pid: i32) -> Option<i32> {
    let function = responsible_function()?;
    let responsible = unsafe { function(pid) };
    (responsible > 0).then_some(responsible)
}

pub(crate) fn responsible_children(host: i32) -> Vec<i32> {
    if responsible_function().is_none() {
        return Vec::new();
    }
    crate::system::process::process_arguments::list_pids(
        crate::system::process::process_arguments::PROC_ALL_PIDS,
        0,
    )
    .unwrap_or_default()
    .into_iter()
    .filter(|pid| *pid > 0 && *pid != host && responsible_pid(*pid) == Some(host))
    .collect()
}

pub(crate) fn window_owners(host: i32) -> Result<WindowOwners, AdapterError> {
    use super::cg_window::{WindowRecordScope, records_from_dictionaries, window_dictionaries};

    let dictionaries = window_dictionaries()?;
    let owners = dictionaries
        .iter()
        .filter_map(|dictionary| super::cg_window::int_field(dictionary, "kCGWindowOwnerPID"))
        .filter_map(|pid| i32::try_from(pid).ok())
        .collect();
    let host_windows = records_from_dictionaries(dictionaries, WindowRecordScope::Pid(host))?
        .into_iter()
        .filter(|record| record.visible)
        .map(|record| record.bounds)
        .collect();
    Ok(WindowOwners {
        host_windows,
        owners,
    })
}

fn responsible_function() -> Option<ResponsiblePid> {
    static FUNCTION: OnceLock<Option<ResponsiblePid>> = OnceLock::new();
    *FUNCTION.get_or_init(|| {
        let symbol = unsafe {
            libc::dlsym(
                libc::RTLD_DEFAULT,
                c"responsibility_get_pid_responsible_for_pid".as_ptr(),
            )
        };
        (!symbol.is_null())
            .then(|| unsafe { std::mem::transmute::<*mut c_void, ResponsiblePid>(symbol) })
    })
}

#[cfg(test)]
#[path = "hosted_focus_process_tests.rs"]
mod tests;
