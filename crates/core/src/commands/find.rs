use crate::{
    AppError, IdentityPredicate, LocatorQuery, StatePredicate, adapter::PlatformAdapter,
    context::CommandContext, tree::search_text,
};
use serde_json::Value;

const DEFAULT_LIMIT: usize = 50;

/// Match-criteria fields: how a candidate element is identified. Grouped out
/// of [`FindArgs`] to keep it under the repo's field-count limit.
pub struct FindFilterArgs {
    pub role: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub native_id: Option<String>,
    pub value: Option<String>,
    pub text: Option<String>,
    pub exact: bool,
}

/// Result-shaping fields: which of the matches to return. Mutually exclusive
/// at the CLI/batch layer (enforced by [`validate_find_mode`]).
pub struct FindSelectionArgs {
    pub count: bool,
    pub first: bool,
    pub last: bool,
    pub nth: Option<usize>,
    pub limit: Option<usize>,
    /// Return the matches an incomplete walk observed instead of a timeout.
    pub allow_partial: bool,
}

/// Where to search: the application or window, an optional root ref inside
/// it, and the surface. Grouped out of [`FindArgs`] to keep it under the
/// repo's field-count limit.
pub struct FindScopeArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub root: Option<String>,
    pub surface: crate::SnapshotSurface,
    /// Page the root's scroll area toward this direction, then back, until a
    /// match loads.
    pub scroll: Option<crate::Direction>,
}

pub struct FindArgs {
    pub scope: FindScopeArgs,
    pub filter: FindFilterArgs,
    pub states: Vec<StatePredicate>,
    pub selection: FindSelectionArgs,
    pub timeout_ms: Option<u64>,
}

pub fn execute(
    args: FindArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    validate_find_mode(&args)?;
    super::surface_scope::reject_root_with_surface(
        "find",
        args.scope.root.as_deref(),
        args.scope.surface,
    )?;
    adapter.check_surface_support(args.scope.surface)?;
    let query = locator_query_from_args(&args)?;
    query.validate_states().map_err(AppError::Adapter)?;

    if args.scope.scroll.is_some() {
        return find_scroll::execute(&args, &query, adapter, context)
            .map_err(|error| name_the_traversal_remedy(error, args.scope.surface));
    }
    live::execute(&args, &query, adapter, context)
        .map_err(|error| name_the_traversal_remedy(error, args.scope.surface))
}

/// A traversal that ran out of budget on a large tree is not evidence the
/// target is unresponsive, which is what the shared timeout suggestion says.
/// The caller has two real levers here - a bigger budget, or a narrower
/// search - so this names them instead of sending them to inspect a healthy
/// app. Applied at this boundary rather than in the shared constructor,
/// which every other command reaches and whose wording is asserted on
/// elsewhere.
fn name_the_traversal_remedy(error: AppError, surface: crate::SnapshotSurface) -> AppError {
    let AppError::Adapter(adapter_error) = error else {
        return error;
    };
    if adapter_error.code != crate::ErrorCode::Timeout {
        return AppError::Adapter(adapter_error);
    }
    if surface == crate::SnapshotSurface::Menubar && left_a_part_unreadable(&adapter_error) {
        return AppError::Adapter(name_the_unreadable_part(adapter_error));
    }
    AppError::Adapter(adapter_error.with_suggestion(
        "The search ran out of budget before it could answer. Raise --timeout-ms, or narrow the search with --root, --surface, or a more specific role and name.",
    ))
}

/// A walk that ended incomplete with no budget limit and no transient read
/// failure stopped at a subtree the app would not read out, such as a field
/// whose children fail to load. More time does not read it.
fn left_a_part_unreadable(error: &crate::AdapterError) -> bool {
    error.details.as_ref().is_some_and(|details| {
        let stats = &details["query_stats"];
        let none = |value: &serde_json::Value, keys: &[&str]| {
            keys.iter().all(|key| value[*key].as_u64() == Some(0))
        };
        details["kind"] == "locator_incomplete"
            && details["retryable"] != true
            && none(
                &stats["traversal"]["limits"],
                &[
                    "node_hits",
                    "edge_hits",
                    "child_hits",
                    "text_hits",
                    "depth_hits",
                    "child_count_changes",
                ],
            )
            && none(
                &stats["reads"],
                &[
                    "cannot_complete",
                    "deadline_exhausted",
                    "invalidated_elements",
                ],
            )
    })
}

/// With no match in the part it read, the search cannot say whether the
/// unread part holds one, so it refuses; with matches, only a count or a
/// positional pick reaches here, which the unread part could change.
fn name_the_unreadable_part(error: crate::AdapterError) -> crate::AdapterError {
    let observed = error
        .details
        .as_ref()
        .and_then(|details| details["observed_matches"].as_u64())
        .unwrap_or(0);
    let mut details = error.details.unwrap_or_else(|| serde_json::json!({}));
    details["kind"] = "locator_unreadable_part".into();
    details["complete"] = false.into();
    let refused = if observed == 0 {
        crate::AdapterError::new(
            crate::ErrorCode::ElementNotFound,
            "No match in the part of the surface that could be read; another part could not be read",
        )
        .with_suggestion(
            "The app returned an error for part of the tree, so a larger --timeout-ms does not help. Search where the element lives, such as the window (drop --surface), or pass --allow-partial to accept the answer for the readable part.",
        )
    } else {
        crate::AdapterError::new(
            crate::ErrorCode::Timeout,
            "Part of the surface could not be read, so a count or positional pick is unproven",
        )
        .with_suggestion(
            "The app returned an error for part of the tree, so a larger --timeout-ms does not help. Run find without --count, --first, --last or --nth to get the matches it observed, marked complete: false, or pass --allow-partial.",
        )
    };
    refused
        .with_details(details)
        .with_disposition(error.disposition)
}

pub fn parse_state_flag(raw: &str) -> Result<StatePredicate, AppError> {
    let (token, expected) = match raw.split_once('=') {
        Some((token, value)) => {
            let expected = match value {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => {
                    return Err(AppError::invalid_input_with_suggestion(
                        format!("Invalid state flag value '{value}' in '{raw}'"),
                        "Use TOKEN, TOKEN=true, or TOKEN=false.",
                    ));
                }
            };
            (token, expected)
        }
        None => (raw, None),
    };
    Ok(StatePredicate {
        token: token.to_string(),
        expected,
    })
}

fn locator_query_from_args(args: &FindArgs) -> Result<LocatorQuery, AppError> {
    Ok(LocatorQuery {
        identity: IdentityPredicate {
            role: args.filter.role.clone(),
            name: args.filter.name.as_deref().map(search_text::normalize),
            description: args
                .filter
                .description
                .as_deref()
                .map(search_text::normalize),
            native_id: args.filter.native_id.clone(),
            value: args.filter.value.as_deref().map(search_text::normalize),
        },
        has_text: args.filter.text.as_deref().map(search_text::normalize),
        exact: args.filter.exact,
        states: args.states.clone(),
        ..LocatorQuery::default()
    })
}

fn validate_find_mode(args: &FindArgs) -> Result<(), AppError> {
    let selector_count = [
        args.selection.count,
        args.selection.first,
        args.selection.last,
        args.selection.nth.is_some(),
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if selector_count > 1 || (selector_count == 1 && args.selection.limit.is_some()) {
        return Err(AppError::invalid_input_with_suggestion(
            "find accepts only one result-shaping mode",
            "Use one of --count, --first, --last, --nth, or --limit.",
        ));
    }
    Ok(())
}

#[path = "find_live.rs"]
mod live;

#[path = "find_response.rs"]
mod find_response;

#[path = "find_scroll.rs"]
mod find_scroll;

#[path = "find_scroll_report.rs"]
mod find_scroll_report;

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "find_live_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "find_rooted_tests.rs"]
mod rooted_tests;

#[cfg(test)]
#[path = "find_unreadable_tests.rs"]
mod unreadable_tests;

#[cfg(test)]
#[path = "find_live_tests.rs"]
mod live_tests;

#[cfg(test)]
#[path = "find_exact_name_tests.rs"]
mod exact_name_tests;

#[cfg(test)]
#[path = "find_first_cut_tests.rs"]
mod first_cut_tests;
