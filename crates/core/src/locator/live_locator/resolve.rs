use super::resolve_errors::{
    deterministic_limit_error, has_deterministic_limit, has_transient_incompleteness,
    incomplete_evidence, transient_incomplete_timeout, unknown_incomplete_error,
};
use super::{
    LocatorResolution, LocatorResolveRequest, LocatorStats, ObservationRequest, ObservationRoot,
    ObservedObservation, RowReading, validate_query, validate_request,
};
use crate::{AppError, adapter::PlatformAdapter, locator::LocatorQuery};
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
/// An incomplete attempt that names no cause, neither a deterministic limit
/// nor a transient read failure, gets one more walk while its observation
/// budget lasts: a cause the platform did not record is not evidence that the
/// next walk cannot finish. When no walk completes, the first such attempt is
/// what fails closed, so its evidence is never replaced by a timeout.
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

/// Resolves through every attempt. A `--first` answer that waited for the
/// row pass to prove its ref is kept as the answer, without that ref, when
/// the row pass fails, so the extra pass never costs a found match.
fn resolve_attempts(
    attempt: &Attempt<'_>,
    observe: impl FnMut(crate::Deadline, RowReading) -> Result<ObservedObservation, AppError>,
) -> Result<LocatorResolution, AppError> {
    let started = crate::wait::deadline::now();
    let mut answer_without_ref = None;
    match resolve_attempts_with(attempt, observe, &mut answer_without_ref) {
        Err(error) => match answer_without_ref {
            Some(resolution) => {
                let aggregate = resolution.stats.clone();
                finish_partial(resolution, aggregate, started)
            }
            None => Err(error),
        },
        resolved => resolved,
    }
}

fn resolve_attempts_with(
    attempt: &Attempt<'_>,
    mut observe: impl FnMut(crate::Deadline, RowReading) -> Result<ObservedObservation, AppError>,
    answer_without_ref: &mut Option<LocatorResolution>,
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
    let mut causeless = None;
    let mut rows = first_pass_rows(request);
    loop {
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            if let Some(partial) = last_partial.take() {
                return finish_partial(partial, aggregate, started);
            }
            if let Some(resolution) = causeless.take() {
                return finish_causeless(attempt.allow_partial, resolution, aggregate, started);
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
            Ok((mut resolution, rows_left_unread, awaits_row_proof)) => {
                resolution.stats.reads.counts.observation_attempts =
                    resolution.stats.reads.counts.observation_attempts.max(1);
                if awaits_row_proof {
                    aggregate.merge_attempt(&resolution.stats);
                    if attempt_deadline.is_expired() {
                        return finish_partial(resolution, aggregate, started);
                    }
                    *answer_without_ref = Some(resolution);
                    rows = RowReading::All;
                    continue;
                }
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
                    aggregate.merge_attempt(&resolution.stats);
                    if causeless.is_none() && !attempt_deadline.is_expired() {
                        causeless = Some(resolution);
                        continue;
                    }
                    return finish_causeless(attempt.allow_partial, resolution, aggregate, started);
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
/// match there, the next pass reads the rows too. So does a match whose ref
/// would carry no element token, since only a count of the whole window
/// proves such a ref.
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
    aggregate.elapsed_us = elapsed_us_since(started);
    resolution.stats = aggregate;
    Ok(resolution)
}

/// Ends a resolution whose walk stayed incomplete with no recorded cause: the
/// observed matches when partial answers are allowed, `locator_incomplete`
/// otherwise, both with the stats of every walk.
fn finish_causeless(
    allow_partial: bool,
    mut resolution: LocatorResolution,
    mut aggregate: LocatorStats,
    started: Instant,
) -> Result<LocatorResolution, AppError> {
    if allow_partial {
        return finish_partial(resolution, aggregate, started);
    }
    aggregate.elapsed_us = elapsed_us_since(started);
    resolution.stats = aggregate;
    Err(unknown_incomplete_error(&resolution).into())
}

fn requires_authoritative_result(request: &LocatorResolveRequest) -> bool {
    request.materialization == super::LocatorMaterialization::SelectedMatches
        || !matches!(
            request.selection,
            super::LocatorSelection::All { .. } | super::LocatorSelection::Count
        )
}

pub(super) fn elapsed_us_since(started: Instant) -> u64 {
    let elapsed = crate::wait::deadline::now().saturating_duration_since(started);
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
}

fn resolve_observed_attempt(
    attempt: &Attempt<'_>,
    observed: ObservedObservation,
    aggregate: &mut LocatorStats,
) -> Result<(LocatorResolution, bool, bool), AppError> {
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
    let awaits_proof = rows_left_unread
        && !unanswered
        && request.materialization == super::LocatorMaterialization::SelectedMatches
        && super::row_proof::awaits_row_proof(&resolution, &tree);
    let published_as_partial =
        (attempt.allow_partial && unanswered && !rows_left_unread) || awaits_proof;
    if published_as_partial {
        super::partial::publish_observed_matches(adapter, query, request, &mut resolution, &tree)?;
    } else if request.materialization == super::LocatorMaterialization::SelectedMatches
        && !(rows_left_unread && unanswered)
    {
        if let Err(error) =
            super::hydrate::selected_matches(adapter, query, request, &mut resolution, &tree)
        {
            aggregate.merge_attempt(&resolution.stats);
            return Err(error);
        }
    }
    Ok((resolution, rows_left_unread, awaits_proof))
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
