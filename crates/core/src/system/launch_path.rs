use serde::{Deserialize, Serialize};

/// Which route a launch took. `AttachedRunning` and `ActivatedRunning` never
/// send the application a reopen event, so an application with no window is
/// left without one. `Reopened` and `Launched` go through the platform's
/// application launcher, which is the only route that can open a window the
/// caller did not ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPath {
    AttachedRunning,
    ActivatedRunning,
    Launched,
    Reopened,
}
