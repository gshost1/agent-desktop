use crate::{AdapterError, ErrorCode, SnapshotSurface};

impl AdapterError {
    /// Records a complete lookup that proved the requested surface absent.
    pub fn surface_absent(surface: SnapshotSurface, message: &str) -> Self {
        let suggestion = if surface == SnapshotSurface::Menu {
            "Open the menu, then take a fresh snapshot with --surface menu".to_owned()
        } else {
            format!(
                "Ensure the application exposes the {} surface, then take a fresh snapshot with --surface {}",
                surface.as_str(),
                surface.as_str(),
            )
        };
        Self::new(ErrorCode::ElementNotFound, message)
            .with_suggestion(suggestion)
            .with_details(serde_json::json!({
                "kind": "surface_absent",
                "complete": true,
                "surface": surface.as_str(),
            }))
    }

    /// A ref taken in a menu that is no longer open. Refreshing cannot bring
    /// it back: the menu must be reopened and snapshotted again.
    pub fn menu_closed() -> Self {
        Self::stale_ref_because("The menu the ref was taken in is no longer open")
            .with_suggestion("Reopen the menu through its opener, take a fresh menu snapshot, and use the new ref. Refreshing a closed menu cannot restore this ref.")
            .with_details(serde_json::json!({
                "kind": "menu_closed",
                "complete": true,
                "retryable": false,
            }))
    }

    /// What a saved ref reports when the surface it was taken in could not
    /// be observed: a complete lookup that found its menu closed means no
    /// live element can match, so the ref is stale; any other failure is
    /// returned as it is.
    pub(crate) fn for_saved_ref_surface(self, surface: SnapshotSurface) -> Self {
        if surface == SnapshotSurface::Menu && self.is_confirmed_surface_absence(surface) {
            Self::menu_closed()
        } else {
            self
        }
    }

    pub fn is_confirmed_surface_absence(&self, surface: SnapshotSurface) -> bool {
        self.code == ErrorCode::ElementNotFound
            && self.details.as_ref().is_some_and(|details| {
                details["kind"] == "surface_absent"
                    && details["complete"] == true
                    && details["surface"] == surface.as_str()
            })
    }
}

#[cfg(test)]
#[path = "surface_absence_tests.rs"]
mod tests;
