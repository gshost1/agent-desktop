use super::{
    evidence::ActionabilityEvidence, gates, receives_events::receives_events_check,
    report::ActionabilityReport, requirements::ActionabilityRequirements,
    stability::StabilityExpectation,
};
use crate::{
    AdapterError, ErrorCode,
    actions::action_request::ActionRequest,
    adapter::{NativeHandle, PlatformAdapter},
};

#[cfg(test)]
use crate::refs::RefEntry;

#[cfg(test)]
pub(crate) fn check(
    entry: &RefEntry,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    use crate::{
        adapter::hit_test::HitTestResult,
        adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    };

    struct TestHitAdapter;

    impl ObservationOps for TestHitAdapter {
        fn hit_test(
            &self,
            _handle: &NativeHandle,
            _point: crate::Point,
            _deadline: crate::Deadline,
        ) -> Result<HitTestResult, AdapterError> {
            Ok(HitTestResult::ReachesTarget)
        }
    }

    impl ActionOps for TestHitAdapter {}
    impl InputOps for TestHitAdapter {}
    impl SystemOps for TestHitAdapter {}

    let evidence = ActionabilityEvidence {
        nonspatial: false,
        state: crate::ElementState {
            role: entry.identity.role.clone(),
            states: entry.capabilities.states.clone(),
            value: entry.identity.value.clone(),
            enabled: Some(!crate::tree::state::has_state(
                &entry.capabilities.states,
                crate::tree::state::DISABLED,
            )),
            hidden: Some(crate::tree::state::has_state(
                &entry.capabilities.states,
                crate::tree::state::HIDDEN,
            )),
            offscreen: Some(crate::tree::state::has_state(
                &entry.capabilities.states,
                crate::tree::state::OFFSCREEN,
            )),
        },
        states_complete: true,
        bounds: entry.geometry.bounds,
        available_actions: entry.capabilities.available_actions.clone(),
    };
    let handle = NativeHandle::null();
    let adapter = TestHitAdapter;
    check_with_stability(
        StabilityExpectation::permissive(entry.geometry.bounds_hash),
        &evidence,
        request,
        Some((&handle, &adapter)),
        crate::Deadline::standard()?,
    )
}

pub(super) fn check_with_stability(
    stability: StabilityExpectation,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    let report = build_report(stability, evidence, request, hit_test, deadline)?;
    super::refusal::finish(evidence, request, report, deadline)
}

/// Preserves stability gaps and positive-timeout application-pointer geometry gaps for sampling.
/// Other unmet requirements remain errors.
pub(super) fn check_with_stability_or_gap(
    stability: StabilityExpectation,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    let (report, requirement) =
        build_report_with_requirement(stability, evidence, request, hit_test, deadline)?;
    if !report.actionable && report.only_blocking_check_is_stability() {
        return Ok(report);
    }
    if evidence.can_wait_for_geometry(request, &report, requirement) {
        return Ok(report);
    }
    super::refusal::finish(evidence, request, report, deadline)
}

pub(super) fn check_before_focus(
    stability: StabilityExpectation,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    let (report, requirement) =
        build_report_with_requirement(stability, evidence, request, hit_test, deadline)?;
    if evidence.can_wait_for_geometry(request, &report, requirement) {
        return Ok(report);
    }
    super::refusal::finish(evidence, request, report, deadline)
}

pub(super) fn build_report(
    stability: StabilityExpectation,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
    deadline: crate::Deadline,
) -> Result<ActionabilityReport, AdapterError> {
    build_report_with_requirement(stability, evidence, request, hit_test, deadline)
        .map(|(report, _)| report)
}

fn build_report_with_requirement(
    stability: StabilityExpectation,
    evidence: &ActionabilityEvidence,
    request: &ActionRequest,
    hit_test: Option<(&NativeHandle, &dyn PlatformAdapter)>,
    deadline: crate::Deadline,
) -> Result<(ActionabilityReport, Option<crate::HeadedRequirement>), AdapterError> {
    let requirement = if request.policy.is_headed() {
        hit_test
            .map(|(handle, adapter)| adapter.headed_requirement(handle, request, deadline))
            .transpose()?
    } else {
        None
    };
    let mut semantic_request;
    let request = if requirement == Some(crate::HeadedRequirement::FocusedApplication) {
        semantic_request = request.clone();
        semantic_request.policy = crate::InteractionPolicy::headless();
        &semantic_request
    } else {
        request
    };
    let requirements = ActionabilityRequirements::for_action(&request.action);
    let protocol = super::protocol_delivery::applies(evidence, request, hit_test);
    let pointer_delivery = if protocol {
        super::PointerDelivery::Semantic
    } else {
        requirements.pointer_delivery(&request.action, &evidence.available_actions, request.policy)
    };
    let mut checks = Vec::new();
    let mut verified_point = None;
    let presentation_point = evidence.bounds.and_then(center_point);
    let without_pointer = requirements.delivered_without_pointer(
        request,
        &evidence.available_actions,
        pointer_delivery,
    );
    if requirements.visible {
        let requires_geometry =
            !protocol && requirements.requires_geometry(request, &evidence.available_actions);
        let visibility = gates::visibility(evidence, requires_geometry, without_pointer);
        if requires_geometry
            && evidence.bounds.is_none()
            && matches!(
                visibility.status,
                super::status::ActionabilityStatus::Unknown
            )
            && let Some((handle, adapter)) = hit_test
            && let Some(refusal) = adapter.pointer_position_refusal(handle, deadline)
        {
            return Err(refusal);
        }
        checks.push(visibility);
    }
    if requirements.requires_stability(pointer_delivery) {
        checks.push(if without_pointer {
            gates::stability_without_pointer(evidence, stability)
        } else {
            gates::stability(stability, evidence.bounds)
        });
    }
    if requirements.enabled {
        checks.push(gates::enabled(evidence));
    }
    if requirements.editable {
        checks.push(gates::editable(evidence, &request.action));
    }
    checks.push(if protocol {
        super::check_result::pass("supported_action")
    } else {
        gates::action_supported(evidence, request)
    });
    checks.push(gates::policy(request));
    if checks
        .iter()
        .any(|check| !matches!(check.status, super::status::ActionabilityStatus::Pass))
    {
        return Ok((
            ActionabilityReport::from_checks(
                checks,
                verified_point,
                presentation_point,
                evidence.bounds,
                pointer_delivery,
            ),
            requirement,
        ));
    }
    if matches!(pointer_delivery, super::PointerDelivery::Physical) {
        match hit_test {
            Some((handle, adapter)) => {
                let (check, point) =
                    receives_events_check(evidence.bounds, handle, adapter, request, deadline)?;
                checks.push(check);
                verified_point = point;
            }
            None => checks.push(super::check_result::unknown(
                "receives_events",
                "live hit-test context unavailable",
            )),
        }
    }
    Ok((
        ActionabilityReport::from_checks(
            checks,
            verified_point,
            presentation_point,
            evidence.bounds,
            pointer_delivery,
        ),
        requirement,
    ))
}

fn center_point(bounds: crate::Rect) -> Option<crate::Point> {
    bounds.validate().ok().and_then(|bounds| {
        (bounds.width > 0.0 && bounds.height > 0.0).then_some(crate::Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        })
    })
}

#[cfg(test)]
#[path = "evaluate_recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
#[path = "position_refusal_tests.rs"]
mod position_refusal_tests;

#[cfg(test)]
#[path = "evaluate_geometry_tests.rs"]
mod geometry_tests;
