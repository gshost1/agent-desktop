use super::display_mutter::LogicalMonitor;

/// Mutter's current monitor configuration: its logical monitors and whether
/// it lays them out in logical pixels (`layout-mode` 1) or physical ones
/// (2, or no mode at all). `None` is a mode this code does not know.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MutterState {
    pub(crate) monitors: Vec<LogicalMonitor>,
    pub(crate) layout_is_logical: Option<bool>,
}
