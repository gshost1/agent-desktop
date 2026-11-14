#[cfg(test)]
use super::find_response::format_match;
use super::find_response::format_response;
use super::{DEFAULT_LIMIT, FindArgs};
use crate::{
    AdapterError, AppError, LocatorQuery,
    adapter::PlatformAdapter,
    context::CommandContext,
    locator::live_locator::{
        LocatorMaterialization, LocatorResolution, LocatorResolveRequest, LocatorSelection,
        ObservationRequest, ObservationRoot, ObservedObservation, resolve_query,
        resolve_query_allowing_partial, selects_tokenless_entry,
    },
    refs::RefMap,
    refs::ref_root_read::{RootRead, read_saved_root},
    refs::refs_store::RefStore,
    snapshot,
    trace::trace_artifacts,
};
use serde_json::{Value, json};
use std::time::Duration;

const LOCATOR_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RAW_DEPTH: u8 = 50;

/// The traversal budget: the caller's if they set one, the shipped default
/// otherwise. Named rather than inlined so a test exercises the same
/// selection a caller reaches, instead of restating the arms beside it.
fn traversal_deadline(args: &FindArgs) -> Result<crate::Deadline, AdapterError> {
    match args.timeout_ms {
        Some(timeout_ms) => crate::Deadline::after(timeout_ms),
        None => crate::Deadline::from_duration(LOCATOR_TIMEOUT),
    }
}

pub(super) fn execute(
    args: &FindArgs,
    query: &LocatorQuery,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let deadline = traversal_deadline(args)?;
    let request = resolve_request(args, deadline);
    let mut resolution = match args.root.as_deref() {
        Some(root_ref) => rooted_resolution(args, query, root_ref, &request, (adapter, context))?,
        None => {
            let owners = snapshot::surface_target::resolve_owners(
                adapter,
                args.app.as_deref(),
                args.window_id.as_deref(),
                args.surface,
                deadline,
            )?;
            snapshot::window_capture::first_surface_owner(owners, |owner| {
                let root = owner.observation_root(args.surface);
                if args.selection.allow_partial {
                    resolve_query_allowing_partial(adapter, query, root, &request)
                } else {
                    resolve_query(adapter, query, root, &request)
                }
            })?
            .1
        }
    };
    if !args.selection.allow_partial {
        require_complete(&resolution)?;
    }
    let published = resolution.refmap.take().filter(|refmap| !refmap.is_empty());
    let ref_count = published.as_ref().map(RefMap::len);
    let snapshot_id = match published {
        Some(refmap) => Some(persist_refmap(context, &refmap)?),
        None => None,
    };
    if let Some(snapshot_id) = snapshot_id.as_deref() {
        for found in &mut resolution.matches {
            if let Some(local_ref) = found.data.ref_id.as_deref() {
                found.data.ref_id = Some(crate::refs::ref_token::qualify_ref_id(
                    snapshot_id,
                    local_ref,
                ));
            }
        }
    }
    emit_resolution(context, &resolution, snapshot_id.as_deref(), ref_count)?;
    format_response(args, query, resolution, snapshot_id.as_deref())
}

/// A rooted find draws its matches from the root's own subtree. A root with an
/// element token is read as its subtree alone when every ref that subtree could
/// publish carries a token too, since a token re-identifies itself. Any other
/// root is re-identified over one complete whole-window observation - through
/// its predicate - and the query runs scoped to that node: the refs the find
/// publishes are then proven by the one observation it took, never by counts
/// joined from two reads. It always reads list and table rows: a pass that
/// left them unread could not re-identify a root by counting the window.
fn rooted_resolution(
    args: &FindArgs,
    query: &LocatorQuery,
    root_ref: &str,
    request: &LocatorResolveRequest,
    (adapter, context): (&dyn PlatformAdapter, &CommandContext),
) -> Result<LocatorResolution, AppError> {
    let (_, local_root_ref) = crate::refs::ref_token::resolve_ref_target(root_ref)?;
    let entry = crate::commands::helpers::load_ref_entry(root_ref, context)?;
    let observe = |deadline, _rows: crate::locator::live_locator::RowReading| {
        let read = read_saved_root(
            adapter,
            &entry,
            deadline,
            |_, handle| {
                let root = ObservationRoot::Element {
                    handle,
                    entry: &entry,
                    root_ref: Some(&local_root_ref),
                };
                let observation =
                    ObservationRequest::locator_for_root(query, request, root, deadline)
                        .validate()?;
                crate::tree::renderer_accessibility::observe_tree(adapter, root, &observation, None)
            },
            |tree| !selects_tokenless_entry(tree),
        )?;
        match read {
            RootRead::Subtree(tree) => Ok(ObservedObservation::whole(tree)),
            RootRead::Window(window_read) => {
                let (_, tree, index) = *window_read;
                ObservedObservation::under(tree, index)
            }
        }
    };
    if args.selection.allow_partial {
        crate::locator::live_locator::resolve_query_from_allowing_partial(
            adapter, query, request, observe,
        )
    } else {
        crate::locator::live_locator::resolve_query_from(adapter, query, request, observe)
    }
}

fn resolve_request(args: &FindArgs, deadline: crate::Deadline) -> LocatorResolveRequest {
    let selection = if args.selection.count {
        LocatorSelection::Count
    } else if args.selection.first {
        LocatorSelection::First
    } else if args.selection.last {
        LocatorSelection::Last
    } else if let Some(index) = args.selection.nth {
        LocatorSelection::Nth(u32::try_from(index).unwrap_or(u32::MAX))
    } else {
        LocatorSelection::All {
            limit: args
                .selection
                .limit
                .map_or(Some(DEFAULT_LIMIT as u32), |limit| {
                    (limit != 0).then(|| u32::try_from(limit).unwrap_or(u32::MAX))
                }),
        }
    };
    LocatorResolveRequest {
        selection,
        deadline,
        max_raw_depth: MAX_RAW_DEPTH,
        surface: (args.surface != crate::SnapshotSurface::Window).then_some(args.surface),
        materialization: if args.selection.count {
            LocatorMaterialization::None
        } else {
            LocatorMaterialization::SelectedMatches
        },
    }
}

fn require_complete(resolution: &LocatorResolution) -> Result<(), AppError> {
    if resolution.meta.selection_complete {
        return Ok(());
    }
    Err(
        AdapterError::timeout("Locator traversal did not produce an authoritative result")
            .with_details(json!({
                "kind": "locator_incomplete",
                "observed_matches": resolution.meta.total_matches,
                "query_stats": resolution.stats,
                "roles_present": resolution.meta.roles_present,
            }))
            .into(),
    )
}

fn persist_refmap(context: &CommandContext, refmap: &RefMap) -> Result<String, AppError> {
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = store.save_new_snapshot(refmap)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id, refmap)?;
    Ok(snapshot_id)
}

fn emit_resolution(
    context: &CommandContext,
    resolution: &LocatorResolution,
    snapshot_id: Option<&str>,
    ref_count: Option<usize>,
) -> Result<(), AppError> {
    context.trace_lazy("locator.resolve", || {
        json!({
            "complete": resolution.meta.complete,
            "match_count": resolution.meta.total_matches,
            "query_stats": resolution.stats,
            "ref_count": ref_count,
            "snapshot_id": snapshot_id,
            "truncated": resolution.meta.truncated,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locator::live_locator::{LocatorResolutionMeta, LocatorStats};

    fn args(limit: Option<usize>) -> FindArgs {
        FindArgs {
            app: None,
            window_id: None,
            root: None,
            surface: crate::SnapshotSurface::Window,
            filter: crate::commands::find::FindFilterArgs {
                role: None,
                name: None,
                description: None,
                native_id: None,
                value: None,
                text: None,
                exact: false,
            },
            states: Vec::new(),
            timeout_ms: None,
            selection: crate::commands::find::FindSelectionArgs {
                count: false,
                first: false,
                last: false,
                nth: None,
                limit,
                allow_partial: false,
            },
        }
    }

    /// The budget is the caller's to set. Without this, find is fixed at five
    /// seconds and a large tree - a shell file dialog is the measured case -
    /// has no answer but to fail, while snapshot beside it takes --timeout-ms.
    #[test]
    fn an_explicit_budget_reaches_the_traversal_and_absence_keeps_the_default() {
        let mut explicit = args(None);
        explicit.timeout_ms = Some(30_000);
        let deadline = traversal_deadline(&explicit).expect("deadline");
        assert_eq!(deadline.timeout_ms(), 30_000);

        let defaulted = args(None);
        let deadline = traversal_deadline(&defaulted).expect("deadline");
        assert_eq!(
            deadline.timeout_ms(),
            5_000,
            "omitting the flag must leave the shipped default alone"
        );
    }
    #[test]
    fn context_matches_expose_geometry_without_inventing_action_refs() {
        let data = crate::locator::live_locator::LocatorMatchData {
            ref_id: None,
            role: "group".into(),
            name: "(unnamed group)".into(),
            value: None,
            states: Vec::new(),
            interactive: false,
            path: vec!["list".into()],
        };
        let bounds = crate::Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 30.0,
        };
        let response = format_match(data.clone(), Some(bounds)).unwrap();

        assert_eq!(response["bounds"], json!(bounds));
        assert!(response.get("ref_id").is_none());
        assert!(format_match(data, None).unwrap().get("bounds").is_none());
    }

    #[test]
    fn zero_limit_requests_all_matches() {
        let request = resolve_request(&args(Some(0)), crate::Deadline::standard().unwrap());
        assert_eq!(request.selection, LocatorSelection::All { limit: None });
    }

    #[test]
    fn all_response_exposes_total_and_truncation() {
        let resolution = LocatorResolution {
            matches: Vec::new(),
            refmap: None,
            stats: LocatorStats::default(),
            meta: LocatorResolutionMeta {
                total_matches: 72,
                complete: true,
                selection_complete: true,
                truncated: true,
                roles_present: vec!["button".into()],
            },
        };
        let response =
            format_response(&args(Some(50)), &LocatorQuery::default(), resolution, None).unwrap();

        assert_eq!(response["total_matches"], 72);
        assert_eq!(response["truncated"], true);
    }
}

#[cfg(test)]
#[path = "find_live_persist_tests.rs"]
mod persist_tests;
