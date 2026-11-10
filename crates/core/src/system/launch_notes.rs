use serde::{Deserialize, Serialize};

use crate::system::launch_path::LaunchPath;

/// What a launch tells the caller beyond the process it reached: the route it
/// took and any guidance for driving the application next. Flattened into the
/// launch result, so both keys sit beside `app` and `pid`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaunchNotes {
    /// Guidance for the calling agent on how to drive this application,
    /// set by core from `renderer` and whether `cdp` is present. Absent
    /// when there is nothing more useful to say than the fields already do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// The route the launch took, absent on a platform that has one route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_path: Option<LaunchPath>,
}
