use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowState {
    pub is_focused: bool,
    #[serde(default = "default_accessible")]
    pub accessible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimized: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Whether the window sits on a workspace other than the one on screen,
    /// such as another macOS Space. Absent when the platform does not know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_other_space: Option<bool>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            is_focused: false,
            accessible: true,
            minimized: None,
            visible: None,
            on_other_space: None,
        }
    }
}

const fn default_accessible() -> bool {
    true
}
