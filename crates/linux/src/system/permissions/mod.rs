use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, PermissionReport, PermissionState};

use crate::system::SharedBus;
use crate::system::bus::probe_registry;
use crate::system::display::display_error::is_display_refusal;
use crate::system::display::route::{Route, current};
use crate::system::x11::capture::grab::{DisplayGrabber, Grabber};

/// The most the preflight's display probe may take. It runs before the
/// command's own deadline starts, so it must fit well inside the one second a
/// command may overrun its timeout by, even when the display never answers.
const CAPTURE_PROBE_SLICE: Duration = Duration::from_millis(500);

type FromProbe =
    fn(Result<(), AdapterError>, CaptureAvailability) -> Result<PermissionReport, AdapterError>;

/// The report `status` and `permissions` show, where an unreachable bus is
/// accessibility `denied` with its recovery step.
pub(crate) fn report(
    bus: &SharedBus,
    deadline: Deadline,
) -> Result<PermissionReport, AdapterError> {
    read_report(bus, deadline, report_from_probe)
}

/// The report a command preflight reads. A display refusal is returned as
/// itself instead of a `denied` report, so the command answers with the
/// display's own error rather than a missing permission.
pub(crate) fn preflight_report(
    bus: &SharedBus,
    deadline: Deadline,
) -> Result<PermissionReport, AdapterError> {
    read_report(bus, deadline, preflight_from_probe)
}

fn read_report(
    bus: &SharedBus,
    deadline: Deadline,
    from_probe: FromProbe,
) -> Result<PermissionReport, AdapterError> {
    crate::input::x11::stale_release::sweep();
    let capture =
        capture_availability(current(), &DisplayGrabber, capture_probe_deadline(deadline));
    let probed = bus
        .get(deadline)
        .and_then(|connection| probe_registry(bus, &connection, deadline));
    let report = from_probe(probed, capture)?;
    deadline.ensure_remaining()?;
    Ok(report)
}

fn capture_probe_deadline(deadline: Deadline) -> Deadline {
    deadline.capped(CAPTURE_PROBE_SLICE)
}

/// Whether the accessibility registry answers on the session bus, which is
/// what every snapshot surface reads through.
pub(crate) fn accessibility_answers(bus: &SharedBus, deadline: Deadline) -> bool {
    bus.get(deadline)
        .and_then(|connection| probe_registry(bus, &connection, deadline))
        .is_ok()
}

/// Whether the session's capture route answered. Only an X11 display that
/// answers inside the deadline is `Reachable`; every other case, Wayland
/// included, is `Unknown` because nothing was learned about the route.
pub(crate) enum CaptureAvailability {
    Reachable,
    Unknown,
}

/// Probes the X11 display with one bounded request. A Wayland session, a
/// missing display and a display that does not answer are all `Unknown`; no
/// portal is called.
pub(crate) fn capture_availability(
    route: Result<Route, AdapterError>,
    grabber: &dyn Grabber,
    deadline: Deadline,
) -> CaptureAvailability {
    match route {
        Ok(Route::X11(_)) if grabber.compositing(deadline).is_ok() => {
            CaptureAvailability::Reachable
        }
        _ => CaptureAvailability::Unknown,
    }
}

/// An unreachable bus is the accessibility permission being unavailable, so it
/// is reported as `Denied` with the cause-specific recovery step. A timeout or
/// an unexpected bus error stays an error: nothing was learned about the bus.
pub(crate) fn report_from_probe(
    probed: Result<(), AdapterError>,
    capture: CaptureAvailability,
) -> Result<PermissionReport, AdapterError> {
    let accessibility = match probed {
        Ok(()) => PermissionState::Granted,
        Err(error) if error.code == ErrorCode::PlatformNotSupported => PermissionState::Denied {
            suggestion: error
                .suggestion
                .unwrap_or_else(|| "Start the AT-SPI accessibility bus and retry".into()),
        },
        Err(error) => return Err(error),
    };
    Ok(PermissionReport {
        accessibility,
        screen_recording: match capture {
            CaptureAvailability::Reachable => PermissionState::NotRequired,
            CaptureAvailability::Unknown => PermissionState::Unknown,
        },
        automation: PermissionState::NotRequired,
    })
}

/// [`report_from_probe`] for a command preflight: a bus that failed because
/// the session's display cannot be reached is that display refusal, unchanged.
pub(crate) fn preflight_from_probe(
    probed: Result<(), AdapterError>,
    capture: CaptureAvailability,
) -> Result<PermissionReport, AdapterError> {
    match probed {
        Err(error) if is_display_refusal(&error) => Err(error),
        probed => report_from_probe(probed, capture),
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
