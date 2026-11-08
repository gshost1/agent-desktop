use std::time::Instant;

use super::menu_match::MenuMatch;

/// Everything one bounded pass over the menu bar read. `unread_menus` names
/// each submenu whose items the application did not hand over in time, by its
/// top-level menu and parent item. `unread_after_budget` counts elements still
/// queued when the lookup budget ran out after every top-level menu was read.
/// `slowest_read` is the longest single read and where it happened, which is
/// what a caller needs to see when the walk is cut short.
#[derive(Default)]
pub(crate) struct MenuWalk {
    pub(crate) matches: Vec<MenuMatch>,
    pub(crate) unread_menus: Vec<String>,
    pub(crate) unread_after_budget: usize,
    pub(crate) visited: usize,
    pub(crate) slowest_read: Option<(u128, String)>,
}

impl MenuWalk {
    pub(crate) fn note_read(&mut self, started: Instant, at: impl FnOnce() -> String) {
        let elapsed = started.elapsed().as_millis();
        if self
            .slowest_read
            .as_ref()
            .is_none_or(|(slowest, _)| elapsed > *slowest)
        {
            self.slowest_read = Some((elapsed, at()));
        }
    }

    /// The parts of the walk a press or a skip reports beside its own fields.
    pub(crate) fn report_fields(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut fields = serde_json::Map::new();
        fields.insert("visited".into(), self.visited.into());
        if !self.unread_menus.is_empty() {
            fields.insert("unread_menus".into(), serde_json::json!(self.unread_menus));
        }
        if self.unread_after_budget > 0 {
            fields.insert(
                "unread_after_budget".into(),
                self.unread_after_budget.into(),
            );
        }
        if let Some((ms, at)) = &self.slowest_read {
            fields.insert(
                "slowest_read".into(),
                serde_json::json!({ "ms": ms, "at": at }),
            );
        }
        fields
    }
}

#[cfg(test)]
#[path = "menu_walk_tests.rs"]
mod tests;
