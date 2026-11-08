/// How a chord's menu-bar lookup ended. `Pressed` and `Skipped` carry the
/// `details.menu_shortcut` object the press report shows; `Skipped` means the
/// keyboard path runs even though the lookup applied, and `NotApplicable`
/// leaves the keyboard path exactly as it was.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MenuShortcutOutcome {
    Pressed(serde_json::Value),
    Skipped(serde_json::Value),
    NotApplicable,
}

impl MenuShortcutOutcome {
    pub(crate) fn skipped(reason: &'static str) -> Self {
        Self::Skipped(serde_json::json!({ "skipped": reason }))
    }
}
