use crate::{AdapterError, AppInventory, Deadline, ErrorCode, ObservationOps};

/// Presentation matching includes display-name substrings and exact aliases;
/// mutation lookup requires an exact identifier and optional bundle identity.
#[derive(Debug, Clone, Copy)]
pub enum AppInventoryQuery<'a> {
    All,
    NameOrAlias(&'a str),
    /// Diagnostic suggestions only, not evidence of exact process identity.
    NearMatches(&'a str),
    Exact {
        name: &'a str,
        bundle_id: Option<&'a str>,
    },
}

impl AppInventoryQuery<'_> {
    pub(crate) fn collect(
        self,
        adapter: &(impl ObservationOps + ?Sized),
        deadline: Deadline,
    ) -> Result<AppInventory, AdapterError> {
        let name = match self {
            Self::All => return adapter.list_apps_inventory(deadline),
            Self::Exact { name, bundle_id } => {
                return Ok(AppInventory {
                    apps: adapter.list_apps_scoped(name, bundle_id, deadline)?,
                    skipped: Vec::new(),
                });
            }
            Self::NearMatches(_) => {
                let mut inventory = adapter.list_apps_inventory(deadline)?;
                inventory.apps.retain(|app| {
                    self.matches_display_name(&app.name)
                        || app
                            .bundle_id
                            .as_deref()
                            .is_some_and(|id| self.matches_display_name(id))
                });
                return Ok(inventory);
            }
            Self::NameOrAlias(name) => name,
        };
        let mut inventory = adapter.list_apps_inventory(deadline)?;
        inventory
            .apps
            .retain(|app| self.matches_display_name(&app.name));
        let aliased = match adapter.list_apps_scoped(name, None, deadline) {
            Ok(apps) => apps,
            Err(err) if err.code == ErrorCode::PlatformNotSupported => Vec::new(),
            Err(err) => return Err(err),
        };
        for app in aliased {
            if !inventory.apps.iter().any(|kept| kept.pid == app.pid) {
                inventory.apps.push(app);
            }
        }
        Ok(inventory)
    }

    /// Matches the listed name; adapters add their verified exact aliases.
    pub fn matches_display_name(self, actual: &str) -> bool {
        match self {
            Self::All => true,
            Self::NameOrAlias(name) => crate::tree::search_text::contains(
                actual,
                &crate::tree::search_text::normalize(name),
            ),
            Self::NearMatches(name) => super::app_lookup::is_near_app_name(actual, name),
            Self::Exact { name, .. } => crate::app_name_matches(actual, name),
        }
    }
}

#[cfg(test)]
#[path = "app_inventory_query_tests.rs"]
mod tests;
