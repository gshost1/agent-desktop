//! A menu item belongs to the application's menu bar, not to any of its
//! windows, so a window search for one finds nothing and the caller falls back
//! to reading the whole menu bar. When a `menuitem` find over the default
//! window surface finds nothing, or the application has no window to search,
//! the same query runs once over the menu bar, and the response names the
//! surface that answered it.

use super::FindArgs;
use crate::{
    AppError, ErrorCode, LocatorQuery, SnapshotSurface, adapter::PlatformAdapter,
    context::CommandContext, tree::roles,
};
use serde_json::{Value, json};

const MENU_ITEM_ROLE: &str = "menuitem";

pub(super) fn retry_in_menu_bar(
    mut args: FindArgs,
    query: &LocatorQuery,
    window_result: Result<Value, AppError>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if !searches_window_for_menu_item(&args, query)
        || !window_found_nothing(&window_result)
        || adapter
            .check_surface_support(SnapshotSurface::Menubar)
            .is_err()
    {
        return window_result;
    }
    args.surface = SnapshotSurface::Menubar;
    match super::live::execute(&args, query, adapter, context) {
        Ok(mut menu_result) if !is_empty(&menu_result) => {
            if let Some(object) = menu_result.as_object_mut() {
                object.insert("surface".into(), json!(SnapshotSurface::Menubar.as_str()));
            }
            Ok(menu_result)
        }
        _ => window_result,
    }
}

pub(super) fn searches_window_for_menu_item(args: &FindArgs, query: &LocatorQuery) -> bool {
    args.surface == SnapshotSurface::Window
        && args.root.is_none()
        && query
            .identity
            .role
            .as_deref()
            .is_some_and(|role| roles::normalize_role_query(role) == MENU_ITEM_ROLE)
}

pub(super) fn window_found_nothing(result: &Result<Value, AppError>) -> bool {
    match result {
        Ok(response) => is_empty(response),
        Err(AppError::Adapter(error)) => error.code == ErrorCode::WindowNotFound,
        Err(_) => false,
    }
}

pub(super) fn is_empty(response: &Value) -> bool {
    if let Some(count) = response.get("count") {
        return count.as_u64() == Some(0);
    }
    if let Some(found) = response.get("match") {
        return found.is_null();
    }
    response
        .get("matches")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

#[cfg(test)]
#[path = "find_menu_bar_tests.rs"]
mod tests;
