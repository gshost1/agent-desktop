use crate::AppError;

/// Which flag-selected condition a `wait` targets: `--menu`,
/// `--menu-closed`, `--notification` or `--download`. One variant per flag
/// makes the modes structurally mutually exclusive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceWait {
    Menu,
    MenuClosed,
    Notification,
    Download,
}

impl SurfaceWait {
    pub fn from_flags(
        menu: bool,
        menu_closed: bool,
        notification: bool,
    ) -> Result<Option<Self>, AppError> {
        match (menu, menu_closed, notification) {
            (false, false, false) => Ok(None),
            (true, false, false) => Ok(Some(Self::Menu)),
            (false, true, false) => Ok(Some(Self::MenuClosed)),
            (false, false, true) => Ok(Some(Self::Notification)),
            _ => Err(crate::commands::wait_mode::ambiguous_wait_mode()),
        }
    }
}

impl SurfaceWait {
    /// Adds `--download` to the surface flags already read; a download wait
    /// alongside any other surface flag is ambiguous.
    pub fn with_download(surface: Option<Self>, download: bool) -> Result<Option<Self>, AppError> {
        match (surface, download) {
            (surface, false) => Ok(surface),
            (None, true) => Ok(Some(Self::Download)),
            (Some(_), true) => Err(crate::commands::wait_mode::ambiguous_wait_mode()),
        }
    }
}

#[cfg(test)]
#[path = "wait_surface_tests.rs"]
mod tests;
