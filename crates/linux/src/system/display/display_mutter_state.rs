use super::display_mutter::LogicalMonitor;

/// Mutter's current monitor configuration: its logical monitors and whether
/// it lays them out in logical pixels (`layout-mode` 1) or physical ones.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MutterState {
    pub(crate) monitors: Vec<LogicalMonitor>,
    pub(crate) layout_is_logical: bool,
}
