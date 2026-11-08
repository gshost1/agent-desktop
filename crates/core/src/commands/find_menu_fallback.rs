//! A menu item belongs to an open menu or to the application's menu bar, not
//! to any of its windows, so a window search for one finds nothing and the
//! caller falls back to reading the whole menu bar. When a `menuitem` find over
//! the default window surface matches nothing, or the application has no
//! window to search, the same query runs over the open menu and then the menu
//! bar, each where the platform supports that surface, within the one deadline
//! the caller gave the command. The response names the surface that answered.

use super::FindArgs;
use crate::{
    AppError, ErrorCode, LocatorQuery, Role, SnapshotSurface, adapter::PlatformAdapter,
    context::CommandContext,
};
use serde_json::{Value, json};

const MENU_SURFACES: [SnapshotSurface; 2] = [SnapshotSurface::Menu, SnapshotSurface::Menubar];

pub(super) fn search_menus(
    mut args: FindArgs,
    query: &LocatorQuery,
    window_result: Result<(Value, u32), AppError>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    deadline: crate::Deadline,
) -> Result<Value, AppError> {
    if !searches_window_for_menu_item(&args, query) || !window_found_nothing(&window_result) {
        return window_result.map(|(response, _)| response);
    }
    for surface in MENU_SURFACES {
        if adapter.check_surface_support(surface).is_err() || deadline.is_expired() {
            continue;
        }
        args.surface = surface;
        match super::live::execute(&args, query, adapter, context, deadline) {
            Ok((mut response, total)) if total > 0 => {
                if let Some(object) = response.as_object_mut() {
                    object.insert("surface".into(), json!(surface.as_str()));
                }
                return Ok(response);
            }
            Ok(_) => {}
            Err(error) if is_timeout(&error) => return Err(error),
            Err(_) => {}
        }
    }
    window_result.map(|(response, _)| response)
}

pub(super) fn searches_window_for_menu_item(args: &FindArgs, query: &LocatorQuery) -> bool {
    args.surface == SnapshotSurface::Window
        && args.root.is_none()
        && query.identity.role.as_deref().is_some_and(is_menu_role)
}

/// Menu bar titles and the items under them both normalize to `menuitem`.
fn is_menu_role(role: &str) -> bool {
    crate::tree::roles::normalize_role_query(role) == Role::MenuItem.as_str()
}

pub(super) fn window_found_nothing(result: &Result<(Value, u32), AppError>) -> bool {
    match result {
        Ok((_, total)) => *total == 0,
        Err(AppError::Adapter(error)) => error.code == ErrorCode::WindowNotFound,
        Err(_) => false,
    }
}

fn is_timeout(error: &AppError) -> bool {
    matches!(error, AppError::Adapter(adapter_error) if adapter_error.code == ErrorCode::Timeout)
}

#[cfg(test)]
#[path = "find_menu_fallback_tests.rs"]
mod tests;
