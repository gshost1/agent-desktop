use crate::{AdapterError, AppInventory, AppInventoryQuery, Deadline, ErrorCode};

use super::ObservationOps;

pub(super) fn query(
    adapter: &(impl ObservationOps + ?Sized),
    query: AppInventoryQuery<'_>,
    deadline: Deadline,
) -> Result<AppInventory, AdapterError> {
    let name = match query {
        AppInventoryQuery::All => return adapter.list_apps_inventory(deadline),
        AppInventoryQuery::Exact { name, bundle_id } => {
            return Ok(AppInventory {
                apps: adapter.list_apps_scoped(name, bundle_id, deadline)?,
                skipped: Vec::new(),
            });
        }
        AppInventoryQuery::NameOrAlias(name) => name,
    };
    let mut inventory = adapter.list_apps_inventory(deadline)?;
    inventory
        .apps
        .retain(|app| query.matches_display_name(&app.name));
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
