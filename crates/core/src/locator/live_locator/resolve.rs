use super::{
    LocatorResolution, LocatorResolveRequest, LocatorStats, ObservationRequest, ObservationRoot,
    ObservedObservation, RowReading, validate_query, validate_request,
};
use crate::{AdapterError, AppError, ErrorCode, adapter::PlatformAdapter, locator::LocatorQuery};
use serde_json::json;
use std::time::{Duration, Instant};

pub fn resolve_query(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    root: ObservationRoot<'_>,
    request: &LocatorResolveRequest,
) -> Result<LocatorResolution, AppError> {
    resolve_query_from(
        adapter,
        query,
        request,
        observe_root(adapter, query, root, request),
    )
}

/// Resolves like [`resolve_query`], except that a walk the budget cuts short
/// returns what it observed instead of a timeout. See
/// [`resolve_query_from_allowing_partial`].
pub(crate) fn resolve_query_allowing_partial(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    root: ObservationRoot<'_>,
    request: &LocatorResolveRequest,
) -> Result<LocatorResolution, AppError> {
    let observe = observe_root(adapter, query, root, request);
    resolve_query_from_allowing_partial(adapter, query, request, observe)
}

fn observe_root<'a>(
    adapter: &'a dyn PlatformAdapter,
    query: &'a LocatorQuery,
    root: ObservationRoot<'a>,
    request: &'a LocatorResolveRequest,
) -> impl FnMut(crate::Deadline, RowReading) -> Result<ObservedObservation, AppError> + 'a {
    move |deadline, rows| {
        let observation_request =
            ObservationRequest::locator_for_root(query, request, root, deadline)
                .reading_rows(rows)
                .validate()?;
        crate::tree::renderer_accessibility::observe_tree(adapter, root, &observation_request, None)
            .map(ObservedObservation::whole)
    }
}

struct Attempt<'a> {
    adapter: &'a dyn PlatformAdapter,
    query: &'a LocatorQuery,
    request: &'a LocatorResolveRequest,
    allow_partial: bool,
}

/// Resolves a query over trees the caller observes. A caller that already holds
/// the read its root resolution took hands that same tree back here, so the refs
/// it publishes are proven by one observation rather than by a second one.
///
/// A transiently incomplete attempt is retried only while it ended inside its
/// own observation budget. An attempt that spent that budget was cut by time,
/// and every retry gets less of it, so the timeout is returned at once with the
/// partial evidence instead of spending the hydration reserve on another walk.
pub(crate) fn resolve_query_from(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    request: &LocatorResolveRequest,
    observe: impl FnMut(crate::Deadline, RowReading) -> Result<ObservedObservation, AppError>,
) -> Result<LocatorResolution, AppError> {
    let attempt = Attempt {
        adapter,
        query,
        request,
        allow_partial: false,
    };
    resolve_attempts(&attempt, observe)
}

/// Resolves like [`resolve_query_from`] while a walk can answer, and returns
/// the matches an incomplete walk observed where that one returns a timeout.
///
/// A complete walk resolves exactly as it does there. An incomplete one is
/// still retried while its budget lasts. Once it cannot finish, its resolution
/// keeps `selection_complete: false`, its match count is a lower bound, and
/// only matches the platform re-identifies by element token carry a ref.
pub(crate) fn resolve_query_from_allowing_partial(
    adapter: &dyn PlatformAdapter,
    query: &LocatorQuery,
    request: &LocatorResolveRequest,
    observe: impl FnMut(crate::Deadline, RowReading) -> Result<ObservedObservation, AppError>,
) -> Result<LocatorResolution, AppError> {
    let attempt = Attempt {
        adapter,
        query,
        request,
        allow_partial: true,
    };
    resolve_attempts(&attempt, observe)
}

fn resolve_attempts(
    attempt: &Attempt<'_>,
    mut observe: impl FnMut(crate::Deadline, RowReading) -> Result<ObservedObservation, AppError>,
) -> Result<LocatorResolution, AppError> {
    let (query, request) = (attempt.query, attempt.request);
    validate_query(query)?;
    validate_request(request)?;
    let started = crate::wait::deadline::now();
    let deadline = request.deadline;
    let mut aggregate = LocatorStats::default();
    let mut last_incomplete = None;
    let mut last_partial = None;
    let mut selection_retry_used = false;
    let mut rows = first_pass_rows(request);
    loop {
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            if let Some(partial) = last_partial.take() {
                return finish_partial(partial, aggregate, started);
            }
            return Err(transient_incomplete_timeout(
                deadline,
                &aggregate,
                last_incomplete,
                started,
            )
            .into());
        }
        let attempt_deadline = observation_deadline(request);
        match observe(attempt_deadline, rows)
            .and_then(|observed| resolve_observed_attempt(attempt, observed, &mut aggregate))
        {
            Ok((mut resolution, rows_left_unread)) => {
                resolution.stats.reads.counts.observation_attempts =
                    resolution.stats.reads.counts.observation_attempts.max(1);
                if rows_left_unread
                    && !resolution.meta.selection_complete
                    && !attempt_deadline.is_expired()
                {
                    aggregate.merge_attempt(&resolution.stats);
                    rows = RowReading::All;
                    continue;
                }
                if !resolution.meta.selection_complete && has_deterministic_limit(&resolution.stats)
                {
                    if attempt.allow_partial {
                        aggregate.merge_attempt(&resolution.stats);
                        return finish_partial(resolution, aggregate, started);
                    }
                    return Err(deterministic_limit_error(&resolution).into());
                }
                if !resolution.meta.selection_complete
                    && has_transient_incompleteness(&resolution.stats)
                {
                    last_incomplete = Some(incomplete_evidence(&resolution));
                    aggregate.merge_attempt(&resolution.stats);
                    if attempt.allow_partial {
                        if deadline.is_expired() || attempt_deadline.is_expired() {
                            return finish_partial(resolution, aggregate, started);
                        }
                        last_partial = Some(resolution);
                    } else if deadline.is_expired() || attempt_deadline.is_expired() {
                        return Err(transient_incomplete_timeout(
                            deadline,
                            &aggregate,
                            last_incomplete,
                            started,
                        )
                        .into());
                    }
                    let remaining = deadline.remaining();
                    crate::wait::deadline::sleep(remaining.min(Duration::from_millis(10)));
                    continue;
                }
                if !resolution.meta.selection_complete && requires_authoritative_result(request) {
                    if attempt.allow_partial {
                        aggregate.merge_attempt(&resolution.stats);
                        return finish_partial(resolution, aggregate, started);
                    }
                    return Err(unknown_incomplete_error(&resolution).into());
                }
                aggregate.merge_attempt(&resolution.stats);
                aggregate.elapsed_us = elapsed_us_since(started);
                resolution.stats = aggregate;
                return Ok(resolution);
            }
            Err(AppError::Adapter(error))
                if request.materialization == super::LocatorMaterialization::SelectedMatches
                    && super::hydrate::retryable_error(&error)
                    && !selection_retry_used
                    && !deadline.is_expired() =>
            {
                selection_retry_used = true;
                last_incomplete = Some(json!({
                    "phase": "hydration",
                    "code": error.code.as_str(),
                    "details": error.details,
                }));
                let remaining = deadline.remaining();
                crate::wait::deadline::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(error) => return Err(error),
        }
    }
}

/// A `--first` resolution reads everything outside list and table rows
/// first. Its first match there answers the query without the rows; with no
/// match there, the next pass reads the rows too.
fn first_pass_rows(request: &LocatorResolveRequest) -> RowReading {
    if request.selection == super::LocatorSelection::First {
        RowReading::LeaveUnread
    } else {
        RowReading::All
    }
}

fn finish_partial(
    mut resolution: LocatorResolution,
    mut aggregate: LocatorStats,
    started: Instant,
) -> Result<LocatorResolution, AppError> {
    super::partial::publish_observed_matches(&mut resolution)?;
    aggregate.elapsed_us = elapsed_us_since(started);
    resolution.stats = aggregate;
    Ok(resolution)
}

fn requires_authoritative_result(request: &LocatorResolveRequest) -> bool {
    request.materialization == super::LocatorMaterialization::SelectedMatches
        || !matches!(
            request.selection,
            super::LocatorSelection::All { .. } | super::LocatorSelection::Count
        )
}

fn unknown_incomplete_error(resolution: &LocatorResolution) -> AdapterError {
    AdapterError::timeout("Locator traversal returned incomplete evidence")
        .with_suggestion("Retry with a narrower locator or a larger observation budget")
        .with_details(json!({
            "kind": "locator_incomplete",
            "retryable": false,
            "observed_matches": resolution.meta.total_matches,
            "query_stats": resolution.stats,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
}

pub(super) fn has_deterministic_limit(stats: &LocatorStats) -> bool {
    let limits = &stats.traversal.limits;
    limits.node_hits > 0
        || limits.edge_hits > 0
        || limits.child_hits > 0
        || limits.text_hits > 0
        || limits.depth_hits > 0
}

fn has_transient_incompleteness(stats: &LocatorStats) -> bool {
    stats.traversal.limits.child_count_changes > 0
        || stats.reads.health.cannot_complete > 0
        || stats.reads.health.deadline_exhausted > 0
}

fn deterministic_limit_error(resolution: &LocatorResolution) -> AdapterError {
    AdapterError::new(
        ErrorCode::Timeout,
        "Locator traversal reached a deterministic observation budget",
    )
    .with_suggestion(
        "Use a narrower locator, a shallower scope, or increase the matching observation budget",
    )
    .with_details(json!({
        "kind": "locator_budget_limit",
        "retryable": false,
        "observed_matches": resolution.meta.total_matches,
        "limits": resolution.stats.traversal.limits,
        "query_stats": resolution.stats,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

fn incomplete_evidence(resolution: &LocatorResolution) -> serde_json::Value {
    json!({
        "observed_matches": resolution.meta.total_matches,
        "child_count_changes": resolution.stats.traversal.limits.child_count_changes,
        "cannot_complete": resolution.stats.reads.health.cannot_complete,
        "deadline_exhausted": resolution.stats.reads.health.deadline_exhausted,
    })
}

fn elapsed_us_since(started: Instant) -> u64 {
    let elapsed = crate::wait::deadline::now().saturating_duration_since(started);
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
}

fn transient_incomplete_timeout(
    deadline: crate::Deadline,
    stats: &LocatorStats,
    last_incomplete: Option<serde_json::Value>,
    started: Instant,
) -> AdapterError {
    let stats = LocatorStats {
        elapsed_us: elapsed_us_since(started),
        ..stats.clone()
    };
    AdapterError::timeout("Locator observation did not stabilize before its deadline")
        .with_suggestion(
            "Retry with a larger timeout or narrow the locator to a more stable subtree",
        )
        .with_details(json!({
            "kind": "locator_transient_incomplete",
            "retryable": true,
            "timeout_ms": deadline.timeout_ms(),
            "observation_attempts": stats.reads.counts.observation_attempts,
            "last_incomplete": last_incomplete,
            "query_stats": stats,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
}

fn resolve_observed_attempt(
    attempt: &Attempt<'_>,
    observed: ObservedObservation,
    aggregate: &mut LocatorStats,
) -> Result<(LocatorResolution, bool), AppError> {
    let (adapter, query, request) = (attempt.adapter, attempt.query, attempt.request);
    let ObservedObservation { mut tree, scope } = observed;
    tree.stats.reads.counts.observation_attempts =
        tree.stats.reads.counts.observation_attempts.max(1);
    let evaluation_request = LocatorResolveRequest {
        materialization: match request.materialization {
            super::LocatorMaterialization::SelectedMatches => super::LocatorMaterialization::None,
            other => other,
        },
        ..*request
    };
    let (mut resolution, tree) = super::evaluate::evaluate_locator_tree_in(
        tree,
        scope.as_ref(),
        query,
        &evaluation_request,
    )?;
    let rows_left_unread = tree.rows_unread();
    let unanswered = !resolution.meta.selection_complete;
    let published_as_partial = attempt.allow_partial && unanswered;
    if request.materialization == super::LocatorMaterialization::SelectedMatches
        && !published_as_partial
        && !(rows_left_unread && unanswered)
    {
        if let Err(error) =
            super::hydrate::selected_matches(adapter, query, request, &mut resolution, &tree)
        {
            aggregate.merge_attempt(&resolution.stats);
            return Err(error);
        }
    }
    Ok((resolution, rows_left_unread))
}

fn observation_deadline(request: &LocatorResolveRequest) -> crate::Deadline {
    if request.materialization != super::LocatorMaterialization::SelectedMatches {
        return request.deadline;
    }
    let remaining = request.deadline.remaining();
    let reserve = (remaining / 2).min(Duration::from_secs(1));
    request.deadline.capped(remaining.saturating_sub(reserve))
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn selected_ref_hydration_has_budget_without_extending_the_operation() {
        let request = LocatorResolveRequest {
            selection: super::super::LocatorSelection::First,
            deadline: crate::Deadline::after(60_000).unwrap(),
            max_raw_depth: 50,
            surface: None,
            materialization: super::super::LocatorMaterialization::SelectedMatches,
        };
        let observation = observation_deadline(&request);
        assert!(observation.was_capped());
        assert!(!request.deadline.was_capped());
        assert_eq!(observation.timeout_ms(), request.deadline.timeout_ms());
        let count_request = LocatorResolveRequest {
            materialization: super::super::LocatorMaterialization::None,
            ..request
        };
        assert_eq!(observation_deadline(&count_request), request.deadline);
    }
}
