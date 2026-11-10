use agent_desktop_core::AdapterError;
use agent_desktop_core::system::process_state::ProcessState;

/// Result of one AX responsiveness read, decoupled from the raw AXError so
/// `classify` (below) is testable without a live accessibility tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AxProbeResult {
    Responsive,
    CannotComplete,
}

/// Pure classification: alive/dead + a probe closure in, `ProcessState` out.
/// Kept free of any platform API so the retry threshold (one transient
/// `CannotComplete` must not classify `Unresponsive`; two consecutive must)
/// is unit-testable on every host, not just macOS with a live AX tree.
pub(crate) fn classify(
    pid_alive: bool,
    mut probe: impl FnMut() -> Result<AxProbeResult, AdapterError>,
) -> Result<ProcessState, AdapterError> {
    if !pid_alive {
        return Ok(ProcessState::Exited { code: None });
    }
    Ok(match probe()? {
        AxProbeResult::Responsive => ProcessState::Running,
        AxProbeResult::CannotComplete => match probe()? {
            AxProbeResult::Responsive => ProcessState::Running,
            AxProbeResult::CannotComplete => ProcessState::Unresponsive,
        },
    })
}

pub(crate) fn process_state_impl(
    process: agent_desktop_core::ProcessIdentity,
    deadline: agent_desktop_core::Deadline,
) -> Result<ProcessState, AdapterError> {
    use crate::tree::element_for_pid;

    let pid = crate::system::process::process_identity::to_pid_t(process.pid)?;
    if !crate::system::process::process_identity::matches_instance(pid, &process.instance)? {
        return Ok(ProcessState::Exited { code: None });
    }
    let app = element_for_pid(pid);
    let state = classify(true, || {
        crate::tree::properties::attributes::set_messaging_timeout(&app, deadline)?;
        Ok(ax_probe(&app, deadline))
    })?;
    if crate::system::process::process_identity::matches_instance(pid, &process.instance)? {
        Ok(state)
    } else {
        Ok(ProcessState::Exited { code: None })
    }
}

fn ax_probe(app: &crate::tree::AXElement, deadline: agent_desktop_core::Deadline) -> AxProbeResult {
    use accessibility_sys::{kAXErrorCannotComplete, kAXErrorSuccess, kAXRoleAttribute};
    use core_foundation::{
        base::{CFType, TCFType},
        string::CFString,
    };

    if app.0.is_null() {
        return AxProbeResult::Responsive;
    }
    let cf_attr = CFString::new(kAXRoleAttribute);
    let (err, value) = crate::tree::properties::ax_ipc::copy_attribute_value(
        app,
        cf_attr.as_concrete_TypeRef(),
        deadline,
    );
    if err == kAXErrorCannotComplete {
        return AxProbeResult::CannotComplete;
    }
    if err == kAXErrorSuccess && !value.is_null() {
        unsafe { CFType::wrap_under_create_rule(value) };
    }
    AxProbeResult::Responsive
}

#[cfg(test)]
#[path = "process_state_tests.rs"]
mod tests;
