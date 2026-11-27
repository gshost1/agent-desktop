use crate::wait::{ref_wait::RefWait, window_baseline::WindowBaseline};
use crate::{
    AppError, ErrorCode,
    adapter::{PlatformAdapter, TreeOptions},
    commands::selector_hit::SelectorHit,
    commands::selector_observation::SelectorObservation,
    commands::selector_read::SelectorRead,
    commands::{query, snapshot as snapshot_cmd},
    context::CommandContext,
    refs::refs_store::RefStore,
    snapshot::{self, emit_snapshot_saved},
    system::process_state::ProcessState,
    trace::trace_artifacts,
};
use scope::FollowedScope;
use serde_json::{Value, json};

#[path = "wait_selector_scope.rs"]
pub(crate) mod scope;
#[path = "wait_selector_timeout.rs"]
mod timeout;
use timeout::timeout_response;

pub struct WaitSelectorInput {
    pub query_raw: String,
    pub gone: bool,
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub opts: TreeOptions,
    pub timeout_ms: u64,
}

pub fn execute(
    input: WaitSelectorInput,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute_for_target(input, None, adapter, context)
}

/// Waits after an action on a ref, following it into the windows its process
/// opened since `baseline` was taken.
pub(crate) fn execute_after_action(
    input: WaitSelectorInput,
    entry: &crate::RefEntry,
    baseline: Option<&WindowBaseline>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute_for_target(input, Some(RefWait { entry, baseline }), adapter, context)
}

fn execute_for_target(
    mut input: WaitSelectorInput,
    target: Option<RefWait<'_>>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let query = query::validate_selector(&input.query_raw)?;
    let entry = target.map(|target| target.entry);
    if let Some(entry) = entry {
        input.opts.surface = snapshot::surface_target::ref_observation_surface(adapter, entry);
    }
    let mut following = target
        .filter(|target| target.follows(adapter, input.gone))
        .map(FollowedScope::new);

    let deadline = crate::Deadline::at(crate::wait::deadline::now(), input.timeout_ms)?;
    let mut last_error = None;
    let mut last_built = None;
    let result = poll_selector(
        adapter,
        crate::SignalFilter {
            app: input.app.clone(),
            process: entry.and_then(|entry| entry.process.identity()),
        },
        deadline,
        |poll_deadline| {
            let observed = match following.as_mut() {
                Some(scope) => scope.observe(adapter, (&query, &input.query_raw), poll_deadline),
                None => observe_selector(adapter, &input, entry, &query, poll_deadline),
            };
            let completion = match observed {
                Ok(SelectorRead::Hit(_)) if input.gone => None,
                Ok(SelectorRead::Hit(hit)) => {
                    let (owners, opts) = match hit.owner {
                        Some((owner, surface)) => (
                            Ok(vec![owner]),
                            TreeOptions {
                                surface,
                                ..input.opts
                            },
                        ),
                        None => (
                            resolve_owners(adapter, &input, entry, poll_deadline),
                            input.opts,
                        ),
                    };
                    let opts = reaching(opts, hit.depth);
                    match owners.and_then(|owners| {
                        build_materialization(adapter, &opts, owners, &query, poll_deadline)
                    }) {
                        Ok((true, result)) => {
                            Some(Ok(SelectorCompletion::Matched(Box::new(result))))
                        }
                        Ok((false, result)) => {
                            last_built = Some(result);
                            None
                        }
                        Err(err)
                            if err.code() == ErrorCode::Timeout.as_str()
                                || is_transient_poll_error(&err, input.window_id.as_deref())
                                || is_target_gone_error(&err)
                                || snapshot::surface_target::is_confirmed_surface_absence(
                                    &err,
                                    opts.surface,
                                ) =>
                        {
                            last_error = Some(poll_error_json(&err));
                            None
                        }
                        Err(err) => Some(Err(err)),
                    }
                }
                Ok(SelectorRead::Absent) if input.gone => Some(Ok(SelectorCompletion::Absent)),
                Ok(SelectorRead::Absent) => None,
                Ok(SelectorRead::Incomplete) => {
                    last_error = Some(json!({ "kind": "locator_incomplete" }));
                    None
                }
                Err(err) if scope::is_decided(&err) => Some(Err(err)),
                Err(err) if is_target_gone_error(&err) && input.gone && entry.is_none() => {
                    Some(Ok(SelectorCompletion::Absent))
                }
                Err(err)
                    if err.code() == ErrorCode::Timeout.as_str() && poll_deadline.is_expired() =>
                {
                    return Ok(None);
                }
                Err(err)
                    if err.code() == ErrorCode::Timeout.as_str()
                        || is_transient_poll_error(&err, input.window_id.as_deref())
                        || is_target_gone_error(&err) =>
                {
                    last_error = Some(poll_error_json(&err));
                    None
                }
                Err(err) => Some(Err(err)),
            };
            if let Some(completion) = completion {
                return completion.map(Some);
            }

            let remaining = deadline.remaining();
            if !remaining.is_zero()
                && remaining <= super::wait_scheduler::POLL_INTERVAL
                && last_built.is_none()
            {
                match build_snapshot(adapter, &input, entry, poll_deadline) {
                    Ok(result) => last_built = Some(result),
                    Err(error) => last_error = Some(poll_error_json(&error)),
                }
            }
            Ok(None)
        },
    );
    match result {
        Ok(Some(SelectorCompletion::Matched(result))) => materialized_response(
            *result,
            context,
            (&input, &query),
            deadline.elapsed().as_millis(),
        ),
        Ok(Some(SelectorCompletion::Absent)) => Ok(target_absent_response(
            &input.query_raw,
            deadline.elapsed().as_millis(),
        )),
        Ok(None) => timeout_response(adapter, &input, entry, context, (last_built, last_error)),
        Err(err) if err.code() == ErrorCode::Timeout.as_str() && deadline.is_expired() => {
            timeout_response(
                adapter,
                &input,
                entry,
                context,
                (last_built, Some(poll_error_json(&err))),
            )
        }
        Err(err) => Err(err),
    }
}

enum SelectorCompletion {
    Matched(Box<snapshot::SnapshotResult>),
    Absent,
}

pub(crate) fn poll_selector<T>(
    adapter: &dyn PlatformAdapter,
    filter: crate::SignalFilter,
    deadline: crate::Deadline,
    mut observe: impl FnMut(crate::Deadline) -> Result<Option<T>, AppError>,
) -> Result<Option<T>, AppError> {
    let mut scheduler = super::wait_scheduler::WaitScheduler::new(adapter, &filter, deadline);
    loop {
        if let Some(result) = observe(deadline)? {
            return Ok(Some(result));
        }
        if deadline.is_expired() {
            return Ok(None);
        }
        scheduler.wait(deadline);
        if deadline.is_expired() {
            return Ok(None);
        }
    }
}

fn observe_selector(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    query: &crate::LocatorQuery,
    deadline: crate::Deadline,
) -> scope::Observed {
    let observation = if let Some(entry) = entry {
        SelectorObservation::observe_ref(adapter, entry, query, deadline)?
    } else {
        SelectorObservation::observe_owners(
            adapter,
            resolve_owners(adapter, input, None, deadline)?,
            input.opts.surface,
            query,
            deadline,
        )?
    };
    match observation {
        SelectorObservation::Selector {
            complete: false, ..
        } => Ok(SelectorRead::Incomplete),
        SelectorObservation::Selector {
            matches: true,
            depth,
            ..
        } => Ok(SelectorRead::Hit(SelectorHit::on_target(depth))),
        SelectorObservation::Selector { .. }
        | SelectorObservation::OwnerUnavailable(
            ProcessState::Exited { .. } | ProcessState::Crashed { .. },
        ) => Ok(SelectorRead::Absent),
        SelectorObservation::OwnerUnavailable(state) => Err(owner_unavailable(state)),
    }
}

/// The options of the snapshot a matched wait returns, deep enough to hold the
/// match its read found. The read looks further down than a snapshot's
/// default depth, so a match below it deepens this snapshot to the match; a
/// skeleton presents three levels at most, so one below that is returned as
/// a full tree to the match's depth.
fn reaching(opts: TreeOptions, depth: u8) -> TreeOptions {
    if crate::locator::live_locator::ObservationRequest::snapshot_logical_depth(&opts) >= depth {
        return opts;
    }
    TreeOptions {
        max_depth: depth,
        skeleton: false,
        ..opts
    }
}

fn build_materialization(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    owners: Vec<crate::SnapshotOwner>,
    query: &crate::LocatorQuery,
    deadline: crate::Deadline,
) -> Result<(bool, snapshot::SnapshotResult), AppError> {
    let result = snapshot::build_for_owners(adapter, opts, owners, deadline)?;
    let matched = query::tree_has_match(&result.tree, query);
    Ok((matched, result))
}

fn resolve_owners(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    deadline: crate::Deadline,
) -> Result<Vec<crate::SnapshotOwner>, AppError> {
    if let Some(entry) = entry {
        use snapshot::ref_owner_observation::RefOwnerObservation;
        return match snapshot::surface_target::resolve_ref_owner(adapter, entry, deadline)? {
            RefOwnerObservation::Present(owner) => Ok(vec![owner]),
            RefOwnerObservation::Absent(_) => Ok(Vec::new()),
            RefOwnerObservation::Unavailable(state) => Err(owner_unavailable(state)),
        };
    }
    snapshot::surface_target::resolve_owners(
        adapter,
        input.app.as_deref(),
        input.window_id.as_deref(),
        input.opts.surface,
        deadline,
    )
}

fn owner_unavailable(state: ProcessState) -> AppError {
    crate::AdapterError::app_unresponsive("Ref owner")
        .with_details(json!({
            "kind": "ref_owner_unavailable",
            "retryable": true,
            "process_state": state.label(),
        }))
        .into()
}

fn build_snapshot(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    deadline: crate::Deadline,
) -> Result<snapshot::SnapshotResult, AppError> {
    let owners = resolve_owners(adapter, input, entry, deadline)?;
    snapshot::build_for_owners(adapter, &input.opts, owners, deadline)
}

/// A refmap cut can leave the matched element out of the saved tree, since the
/// match ran against the tree as observed. `match_cut` says so, beside the
/// `refmap_capped` the snapshot fields already carry.
fn materialized_response(
    mut result: snapshot::SnapshotResult,
    context: &CommandContext,
    (input, query): (&WaitSelectorInput, &crate::LocatorQuery),
    elapsed_ms: u128,
) -> Result<Value, AppError> {
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = result.save_refmap(&store)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id, &result.refmap)?;
    result.bind_snapshot_id(snapshot_id);
    emit_snapshot_saved(context, &result)?;
    let mut body =
        snapshot_cmd::format_snapshot_fields(&result, Some(elapsed_ms), Some(&input.query_raw))?;
    if result.notes.refmap_capped && !query::tree_has_match(&result.tree, query) {
        body["match_cut"] = json!(true);
    }
    Ok(body)
}

fn target_absent_response(query_raw: &str, elapsed_ms: u128) -> Value {
    json!({
        "matched_selector": query_raw,
        "gone": true,
        "target_absent": true,
        "elapsed_ms": elapsed_ms,
    })
}

fn poll_error_json(err: &AppError) -> Value {
    json!({ "code": err.code(), "message": err.to_string() })
}

fn is_target_gone_error(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Adapter(e)
            if matches!(e.code, ErrorCode::AppNotFound | ErrorCode::WindowNotFound)
    )
}

/// Kinds of `ACTION_NOT_SUPPORTED` that say a window exists but has no
/// accessibility tree right now. A poll keeps going through them: the window
/// may publish its element, or come on screen from another Space, before the
/// wait ends.
const WINDOW_TEMPORARILY_WITHOUT_TREE: [&str; 2] = [
    "window_without_accessibility_element",
    "window_on_other_space",
];

pub(crate) fn is_transient_poll_error(err: &AppError, window_id: Option<&str>) -> bool {
    let AppError::Adapter(error) = err else {
        return false;
    };
    error.is_explicitly_retryable()
        || (error.code == ErrorCode::ActionNotSupported
            && error.details.as_ref().is_some_and(|details| {
                details["kind"]
                    .as_str()
                    .is_some_and(|kind| WINDOW_TEMPORARILY_WITHOUT_TREE.contains(&kind))
                    && window_id.is_none_or(|id| details["window_id"].as_str() == Some(id))
            }))
}

#[cfg(test)]
#[path = "wait_selector_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wait_selector_refmap_cap_tests.rs"]
mod refmap_cap_tests;
