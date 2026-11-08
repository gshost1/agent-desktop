/// How a chord's menu-bar lookup ended. `Skipped` names why the keyboard path
/// runs even though the lookup applied; `NotApplicable` leaves the keyboard
/// path exactly as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MenuShortcutOutcome {
    Pressed {
        menu: Option<String>,
        item: Option<String>,
    },
    Skipped(&'static str),
    NotApplicable,
}
