use agent_desktop_core::system::{launch_options::LaunchOptions, launch_path::LaunchPath};

/// How a launch request reaches the application. Only `LaunchServices` can
/// make the application open a window nobody asked for, because Launch
/// Services answers a running application with a reopen event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LaunchPlan {
    AttachRunning,
    ActivateRunning,
    LaunchServices(LaunchPath),
}

pub(crate) fn plan_launch(options: &LaunchOptions, matching_running: usize) -> LaunchPlan {
    if !options.attach_if_running || matching_running != 1 {
        return LaunchPlan::LaunchServices(LaunchPath::Launched);
    }
    if options.reopen {
        LaunchPlan::LaunchServices(LaunchPath::Reopened)
    } else if options.activate {
        LaunchPlan::ActivateRunning
    } else {
        LaunchPlan::AttachRunning
    }
}

#[cfg(test)]
#[path = "launch_plan_tests.rs"]
mod tests;
