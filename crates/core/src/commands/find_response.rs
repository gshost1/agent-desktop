use super::FindArgs;
use crate::{AppError, LocatorQuery, locator::live_locator::LocatorResolution};
use serde_json::{Value, json};

/// Shapes the find response. A find run with `--allow-partial` also says
/// whether its answer is complete; an incomplete one is marked truncated, and
/// its match count is a lower bound.
pub(super) fn format_response(
    args: &FindArgs,
    query: &LocatorQuery,
    resolution: LocatorResolution,
    snapshot_id: Option<&str>,
) -> Result<Value, AppError> {
    let completeness = args
        .selection
        .allow_partial
        .then_some(resolution.meta.selection_complete);
    let mut response = shaped_response(args, query, resolution, snapshot_id)?;
    if let (Some(object), Some(complete)) = (response.as_object_mut(), completeness) {
        object.insert("complete".into(), json!(complete));
        if !complete {
            object.insert("truncated".into(), json!(true));
        }
    }
    Ok(response)
}

fn shaped_response(
    args: &FindArgs,
    query: &LocatorQuery,
    resolution: LocatorResolution,
    snapshot_id: Option<&str>,
) -> Result<Value, AppError> {
    if args.selection.count {
        return Ok(json!({ "count": resolution.meta.total_matches }));
    }
    let total_matches = resolution.meta.total_matches;
    let truncated = resolution.meta.truncated;
    let roles_present = resolution.meta.roles_present;
    let mut found = resolution.matches;
    order_exact_names_first(&mut found, query, |found| found.data.name.as_str());
    let matches = found
        .into_iter()
        .map(|found| format_match(found.data, found.screen_bounds))
        .collect::<Result<Vec<_>, _>>()?;
    if args.selection.first || args.selection.last || args.selection.nth.is_some() {
        let mut response = single_match_response(matches.into_iter().next(), query, roles_present);
        attach_snapshot_id(&mut response, snapshot_id);
        return Ok(response);
    }
    let mut response = json!({
        "matches": matches,
        "total_matches": total_matches,
        "truncated": truncated,
    });
    let is_empty = response["matches"].as_array().is_some_and(Vec::is_empty);
    attach_roles_present(&mut response, is_empty, query, roles_present);
    attach_snapshot_id(&mut response, snapshot_id);
    Ok(response)
}

/// A name query matches by substring, so an item whose title grew to hold
/// another item's name, such as an Edit menu's "Undo Subscript" after a
/// Subscript command, can come before the item the caller named. Matches whose
/// whole name equals the query come first; otherwise the order stays the
/// traversal order. `--first`, `--last` and `--nth` keep traversal order.
pub(super) fn order_exact_names_first<T>(
    items: &mut [T],
    query: &LocatorQuery,
    name_of: impl Fn(&T) -> &str,
) {
    let Some(wanted) = query.identity.name.as_deref() else {
        return;
    };
    if query.exact {
        return;
    }
    items.sort_by_key(|item| crate::tree::search_text::normalize(name_of(item)) != wanted);
}

pub(super) fn format_match(
    data: crate::locator::live_locator::LocatorMatchData,
    bounds: Option<crate::Rect>,
) -> Result<Value, serde_json::Error> {
    let needs_geometry = data.ref_id.is_none();
    let mut value = serde_json::to_value(data)?;
    if needs_geometry && let Some(bounds) = bounds {
        value["bounds"] = serde_json::to_value(bounds)?;
    }
    Ok(value)
}

fn attach_snapshot_id(response: &mut Value, snapshot_id: Option<&str>) {
    if let (Some(object), Some(snapshot_id)) = (response.as_object_mut(), snapshot_id) {
        object.insert("snapshot_id".into(), json!(snapshot_id));
    }
}

fn single_match_response(
    found: Option<Value>,
    query: &LocatorQuery,
    roles_present: Vec<String>,
) -> Value {
    let is_empty = found.is_none();
    let mut response = json!({ "match": found });
    attach_roles_present(&mut response, is_empty, query, roles_present);
    response
}

fn attach_roles_present(
    response: &mut Value,
    is_empty: bool,
    query: &LocatorQuery,
    roles_present: Vec<String>,
) {
    if !is_empty || query.identity.role.is_none() {
        return;
    }
    if let Some(object) = response.as_object_mut() {
        object.insert("roles_present".into(), json!(roles_present));
    }
}

#[cfg(test)]
#[path = "find_response_tests.rs"]
mod tests;
