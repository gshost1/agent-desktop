use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, CommandContext, DeliverySemantics, ErrorCode, InteractionLease, MouseEvent,
};
use serde_json::json;

struct NoPointerAdapter {
    sent: AtomicUsize,
}

impl NoPointerAdapter {
    fn new() -> Self {
        Self {
            sent: AtomicUsize::new(0),
        }
    }
}

impl ObservationOps for NoPointerAdapter {}

impl ActionOps for NoPointerAdapter {}

impl InputOps for NoPointerAdapter {
    fn physical_pointer_refusal(&self) -> Option<AdapterError> {
        Some(
            AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Pointer input is not available in this session",
            )
            .with_details(json!({
                "cause": "wayland_session",
                "physical_delivery_started": false,
            }))
            .with_disposition(DeliverySemantics::not_delivered()),
        )
    }

    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.sent.fetch_add(1, Ordering::SeqCst);
        Ok(crate::adapter::test_report())
    }
}

impl SystemOps for NoPointerAdapter {}

#[test]
fn a_headed_hover_is_refused_before_the_cursor_is_ever_moved() {
    let adapter = NoPointerAdapter::new();
    let error = hover::execute(
        hover::HoverArgs {
            ref_id: None,
            xy: Some((10.0, 10.0)),
            duration_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .expect_err("a session without pointer input must refuse the hover");

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
    assert_eq!(adapter.sent.load(Ordering::SeqCst), 0);
}

#[test]
fn a_headed_mouse_move_is_refused_before_the_cursor_is_ever_moved() {
    let adapter = NoPointerAdapter::new();
    let error = mouse_move::execute(
        mouse_move::MouseMoveArgs { x: 10.0, y: 10.0 },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .expect_err("a session without pointer input must refuse the move");

    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
    assert_eq!(adapter.sent.load(Ordering::SeqCst), 0);
}

#[test]
fn a_headed_ref_click_is_refused_before_target_resolution() {
    let adapter = NoPointerAdapter::new();
    let error = click::execute(
        helpers::RefArgs {
            ref_id: "@missing:e1".into(),
            timeout_ms: Some(5_000),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .expect_err("the pointer refusal precedes ref loading and actionability");
    let crate::AppError::Adapter(error) = error else {
        panic!("the adapter's pointer refusal must reach the caller");
    };
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.details.unwrap()["physical_delivery_started"], false);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.sent.load(Ordering::SeqCst), 0);
}

fn select_args() -> select::SelectArgs {
    select::SelectArgs {
        ref_id: "@missing:e1".into(),
        value: "Choice".into(),
        timeout_ms: Some(5_000),
    }
}

#[test]
fn a_headed_select_is_refused_before_target_resolution() {
    let adapter = NoPointerAdapter::new();
    let error = select::execute(
        select_args(),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .expect_err("the pointer refusal precedes ref loading and actionability");
    let crate::AppError::Adapter(error) = error else {
        panic!("the adapter's pointer refusal must reach the caller");
    };
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.details.unwrap()["cause"], "wayland_session");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(adapter.sent.load(Ordering::SeqCst), 0);
}

#[test]
fn a_headless_select_is_not_stopped_by_the_missing_pointer() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let adapter = NoPointerAdapter::new();
    let error = select::execute(select_args(), &adapter, &CommandContext::default())
        .expect_err("the unknown ref is the only refusal of a headless select");
    assert_eq!(error.code(), "SNAPSHOT_NOT_FOUND");
}

/// Delivers a pointer at caller-given coordinates but cannot prove the window
/// under an element, as a Wayland compositor does.
struct CoordinateOnlyAdapter {
    inner: NoPointerAdapter,
    drags: AtomicUsize,
}

impl CoordinateOnlyAdapter {
    fn new() -> Self {
        Self {
            inner: NoPointerAdapter::new(),
            drags: AtomicUsize::new(0),
        }
    }

    fn sent(&self) -> usize {
        self.inner.sent.load(Ordering::SeqCst) + self.drags.load(Ordering::SeqCst)
    }
}

impl ObservationOps for CoordinateOnlyAdapter {}

impl ActionOps for CoordinateOnlyAdapter {}

impl InputOps for CoordinateOnlyAdapter {
    fn physical_pointer_refusal(&self) -> Option<AdapterError> {
        self.inner.physical_pointer_refusal()
    }

    fn coordinate_pointer_refusal(&self) -> Option<AdapterError> {
        None
    }

    fn mouse_event(
        &self,
        event: MouseEvent,
        lease: &InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.inner.mouse_event(event, lease)
    }

    fn drag(
        &self,
        _params: crate::DragParams,
        _lease: &InteractionLease,
    ) -> Result<crate::OperationReport, AdapterError> {
        self.drags.fetch_add(1, Ordering::SeqCst);
        Ok(crate::adapter::test_report())
    }
}

impl SystemOps for CoordinateOnlyAdapter {
    crate::adapter::guarded_interaction_lease!();
}

fn headed() -> CommandContext {
    CommandContext::default().with_headed(true)
}

fn xy_endpoint(x: f64) -> drag::DragEndpoint {
    drag::DragEndpoint {
        ref_id: None,
        xy: Some((x, 10.0)),
    }
}

fn drag_args(from: drag::DragEndpoint, to: drag::DragEndpoint) -> drag::DragArgs {
    drag::DragArgs {
        from,
        to,
        duration_ms: None,
        drop_delay_ms: None,
        timeout_ms: None,
        wait_for_scope: drag::WaitForScope::default(),
    }
}

#[test]
fn coordinate_gestures_reach_a_session_that_only_delivers_coordinates() {
    let adapter = CoordinateOnlyAdapter::new();
    mouse_click::execute(
        mouse_click::MouseClickArgs {
            x: 10.0,
            y: 10.0,
            button: crate::MouseButton::Left,
            count: 1,
            modifiers: Vec::new(),
        },
        &adapter,
        &headed(),
    )
    .expect("mouse-click is addressed by coordinates");
    mouse_move::execute(
        mouse_move::MouseMoveArgs { x: 10.0, y: 10.0 },
        &adapter,
        &headed(),
    )
    .expect("mouse-move is addressed by coordinates");
    mouse_wheel::execute(
        mouse_wheel::MouseWheelArgs {
            x: 10.0,
            y: 10.0,
            dy: 1.0,
            dx: 0.0,
            modifiers: Vec::new(),
        },
        &adapter,
        &headed(),
    )
    .expect("mouse-wheel is addressed by coordinates");
    hover::execute(
        hover::HoverArgs {
            ref_id: None,
            xy: Some((10.0, 10.0)),
            duration_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &headed(),
    )
    .expect("hover --xy is addressed by coordinates");
    drag::execute(
        drag_args(xy_endpoint(10.0), xy_endpoint(40.0)),
        &adapter,
        &headed(),
    )
    .expect("a drag between two coordinates is addressed by coordinates");
    assert_eq!(adapter.sent(), 5);
}

fn assert_element_refusal(error: crate::AppError) {
    let crate::AppError::Adapter(error) = error else {
        panic!("the element-aimed pointer refusal must reach the caller");
    };
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.details.unwrap()["cause"], "wayland_session");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn any_ref_endpoint_keeps_the_element_aimed_refusal() {
    let adapter = CoordinateOnlyAdapter::new();
    let with_ref = || drag::DragEndpoint {
        ref_id: Some("@missing:e1".into()),
        xy: None,
    };
    for (from, to) in [
        (with_ref(), xy_endpoint(40.0)),
        (xy_endpoint(10.0), with_ref()),
    ] {
        assert_element_refusal(
            drag::execute(drag_args(from, to), &adapter, &headed())
                .expect_err("a drag with a ref endpoint must prove its window"),
        );
    }
    assert_element_refusal(
        hover::execute(
            hover::HoverArgs {
                ref_id: Some("@missing:e1".into()),
                xy: None,
                duration_ms: None,
                timeout_ms: None,
            },
            &adapter,
            &headed(),
        )
        .expect_err("hover on a ref must prove its window"),
    );
    assert_element_refusal(
        click::execute(
            helpers::RefArgs {
                ref_id: "@missing:e1".into(),
                timeout_ms: Some(5_000),
            },
            &adapter,
            &headed(),
        )
        .expect_err("a headed ref click must prove its window"),
    );
    assert_element_refusal(
        select::execute(select_args(), &adapter, &headed())
            .expect_err("a headed select must prove its window"),
    );
    assert_eq!(adapter.sent(), 0);
}

#[test]
fn a_session_without_coordinate_input_refuses_coordinate_gestures_unsent() {
    let adapter = NoPointerAdapter::new();
    let error = mouse_click::execute(
        mouse_click::MouseClickArgs {
            x: 10.0,
            y: 10.0,
            button: crate::MouseButton::Left,
            count: 1,
            modifiers: Vec::new(),
        },
        &adapter,
        &headed(),
    )
    .expect_err("the coordinate gate defaults to the element-aimed answer");
    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
    let error = drag::execute(
        drag_args(xy_endpoint(10.0), xy_endpoint(40.0)),
        &adapter,
        &headed(),
    )
    .expect_err("a coordinate drag is refused by the same default");
    assert_eq!(error.code(), "ACTION_NOT_SUPPORTED");
    assert_eq!(adapter.sent.load(Ordering::SeqCst), 0);
}
