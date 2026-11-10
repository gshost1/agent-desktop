//! What an activation action changed: focus, an owned sheet, the target window's title.

pub(crate) mod activation_effect;
#[cfg(target_os = "macos")]
pub(crate) mod activation_sheet;
#[cfg(target_os = "macos")]
pub(crate) mod activation_state;
#[cfg(target_os = "macos")]
pub(crate) mod activation_title;
