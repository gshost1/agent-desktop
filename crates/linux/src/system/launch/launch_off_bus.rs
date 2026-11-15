use std::time::Duration;

use agent_desktop_core::Deadline;

use crate::system::x11::control::{ControlRead, toplevels_of};

/// The longest one window read of the off-bus check may take. It may run as
/// the launch deadline expires, so it is bounded on its own and stays well
/// inside the second a command may overrun its timeout by.
const WINDOW_READ: Duration = Duration::from_millis(250);

/// The environment settings with which a toolkit never loads its
/// accessibility bridge.
const BRIDGE_OFF: [&[u8]; 2] = [b"NO_AT_BRIDGE=1", b"GTK_A11Y=none"];

/// What a launch observed about a spawned process that runs and is not on the
/// accessibility bus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OffBus {
    /// No window this tool can see yet: the application may still be starting.
    #[default]
    Starting,
    /// The process maps a top-level window, so its toolkit is up, and it is
    /// still not on the bus.
    Windowed,
    /// The process's environment turns its accessibility bridge off.
    BridgeOff,
}

impl OffBus {
    /// Whether the evidence shows the process will not join the bus however
    /// long the launch waits.
    pub(crate) fn never_joins(self) -> bool {
        self != Self::Starting
    }
}

/// Reads the evidence for `pid`: its environment, then its managed X11
/// top-level windows, Xwayland's included.
pub(crate) fn observe(pid: u32) -> OffBus {
    let environ = std::fs::read(format!("/proc/{pid}/environ")).ok();
    let windowed = || {
        Deadline::from_duration(WINDOW_READ).is_ok_and(|deadline| {
            matches!(toplevels_of(pid, deadline), ControlRead::Ready(windows) if !windows.is_empty())
        })
    };
    classify(environ.as_deref(), windowed)
}

/// The evidence from an environment block and a window check, which runs only
/// when the environment leaves the bridge on.
pub(crate) fn classify(environ: Option<&[u8]>, windowed: impl FnOnce() -> bool) -> OffBus {
    let bridge_off = environ.is_some_and(|block| {
        block
            .split(|byte| *byte == 0)
            .any(|entry| BRIDGE_OFF.contains(&entry))
    });
    if bridge_off {
        OffBus::BridgeOff
    } else if windowed() {
        OffBus::Windowed
    } else {
        OffBus::Starting
    }
}

#[cfg(test)]
#[path = "launch_off_bus_tests.rs"]
mod tests;
