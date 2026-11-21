use super::*;

const BEFORE: Rect = Rect {
    x: 350.0,
    y: 557.0,
    width: 723.0,
    height: 625.0,
};
const CLAMPED: Rect = Rect {
    y: 30.0,
    height: 470.0,
    ..BEFORE
};
const MOVE: GeometryRequest = GeometryRequest::Move {
    x: 350.0,
    y: -200.0,
};
const RESIZE: GeometryRequest = GeometryRequest::Resize {
    width: 100.0,
    height: 100.0,
};
const TARGET: Rect = Rect {
    y: -200.0,
    ..BEFORE
};

#[test]
fn geometry_verification_tolerates_ax_rounding_only() {
    let rounded = Rect {
        x: 351.0,
        y: -201.0,
        width: 101.0,
        height: 99.0,
    };
    let wrong_size = Rect {
        width: 90.0,
        ..rounded
    };
    assert!(MOVE.matches(rounded));
    assert!(RESIZE.matches(rounded));
    assert!(!RESIZE.matches(wrong_size));
}

#[test]
fn clamped_move_and_resize_fail_with_requested_fields_and_observed_frame() {
    for (request, requested) in [
        (MOVE, json!({ "x": 350.0, "y": -200.0 })),
        (RESIZE, json!({ "width": 100.0, "height": 100.0 })),
    ] {
        let error = wait_with_samples(
            request,
            [
                (0, Some(BEFORE)),
                (10, Some(CLAMPED)),
                (259, Some(CLAMPED)),
                (260, Some(CLAMPED)),
                (270, Some(CLAMPED)),
            ],
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::ActionFailed);
        assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
        assert_eq!(
            error.details.unwrap(),
            json!({
                "cause": "readback_mismatch",
                "requested": requested,
                "observed": CLAMPED,
            })
        );
    }
}

#[test]
fn a_frame_held_just_under_the_quiet_window_can_reach_the_target() {
    wait_with_samples(
        MOVE,
        [
            (0, Some(CLAMPED)),
            (125, Some(CLAMPED)),
            (249, Some(CLAMPED)),
            (250, Some(TARGET)),
        ],
    )
    .unwrap();
}

#[test]
fn the_confirmation_read_can_still_find_the_requested_frame() {
    wait_with_samples(
        MOVE,
        [
            (0, Some(CLAMPED)),
            (250, Some(CLAMPED)),
            (260, Some(TARGET)),
        ],
    )
    .unwrap();
}

#[test]
fn a_missing_read_restarts_the_quiet_window() {
    wait_with_samples(
        MOVE,
        [
            (0, Some(CLAMPED)),
            (200, Some(CLAMPED)),
            (225, None),
            (230, Some(CLAMPED)),
            (260, Some(CLAMPED)),
            (270, Some(CLAMPED)),
            (275, Some(TARGET)),
        ],
    )
    .unwrap();
}

#[test]
fn a_return_to_the_original_frame_restarts_the_quiet_window() {
    wait_with_samples(
        MOVE,
        [
            (0, Some(CLAMPED)),
            (250, Some(CLAMPED)),
            (260, Some(BEFORE)),
            (270, Some(CLAMPED)),
            (280, Some(TARGET)),
        ],
    )
    .unwrap();
}

#[test]
fn an_unchanged_frame_keeps_waiting_until_the_deadline() {
    assert_stable_mismatch_times_out(Some(BEFORE), BEFORE);
}

#[test]
fn maximize_readback_still_requires_the_requested_frame() {
    assert_stable_mismatch_times_out(None, CLAMPED);
}

#[test]
fn unavailable_geometry_still_times_out_after_dispatch() {
    let error = wait_for_geometry_with(
        MOVE,
        Some(BEFORE),
        Deadline::detached_after(20).unwrap(),
        || Ok((None, Instant::now())),
    )
    .unwrap_err();
    assert_timeout(error);
}

fn wait_with_samples(
    request: GeometryRequest,
    samples: impl IntoIterator<Item = (u64, Option<Rect>)>,
) -> Result<(), AdapterError> {
    let start = Instant::now();
    let mut samples = samples.into_iter();
    let result = wait_for_geometry_with(
        request,
        Some(BEFORE),
        Deadline::detached_after(1_000).unwrap(),
        || {
            let (millis, frame) = samples.next().expect("unexpected extra geometry read");
            Ok((frame, start + Duration::from_millis(millis)))
        },
    );
    assert!(samples.next().is_none(), "completed before the final read");
    result
}

fn assert_stable_mismatch_times_out(before: Option<Rect>, observed: Rect) {
    let error =
        wait_for_geometry_with(MOVE, before, Deadline::detached_after(450).unwrap(), || {
            Ok((Some(observed), Instant::now()))
        })
        .unwrap_err();
    assert_timeout(error);
}

fn assert_timeout(error: AdapterError) {
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
