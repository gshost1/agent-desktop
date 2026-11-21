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
            error.suggestion.as_deref(),
            Some("Read the held position and size from 'list-windows'")
        );
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
fn an_unchanged_frame_reports_a_mismatch_after_the_longer_quiet_window() {
    let error = wait_with_samples(
        MOVE,
        [
            (0, Some(BEFORE)),
            (250, Some(BEFORE)),
            (260, Some(BEFORE)),
            (999, Some(BEFORE)),
            (1000, Some(BEFORE)),
            (1010, Some(BEFORE)),
        ],
    )
    .unwrap_err();
    assert_mismatch(error, BEFORE);
}

#[test]
fn a_late_match_can_follow_an_unchanged_frame() {
    wait_with_samples(
        MOVE,
        [
            (0, Some(BEFORE)),
            (250, Some(BEFORE)),
            (260, Some(BEFORE)),
            (900, Some(TARGET)),
        ],
    )
    .unwrap();
}

#[test]
fn a_short_deadline_reports_the_latest_readable_clamped_frame() {
    let mut samples = [BEFORE, CLAMPED]
        .into_iter()
        .chain(std::iter::repeat(CLAMPED));
    let error = wait_for_geometry_with(
        MOVE,
        Some(BEFORE),
        Deadline::detached_after(100).unwrap(),
        || (Ok(samples.next()), Instant::now()),
    )
    .unwrap_err();
    assert_mismatch(error, CLAMPED);
}

#[test]
fn a_read_timeout_preserves_preceding_readback() {
    let mut samples = [
        Ok(Some(CLAMPED)),
        Err(AdapterError::timeout("AX read expired")),
    ]
    .into_iter();
    let error = wait_for_geometry_with(
        MOVE,
        Some(BEFORE),
        Deadline::detached_after(1_000).unwrap(),
        || (samples.next().unwrap(), Instant::now()),
    )
    .unwrap_err();
    assert_unavailable(error, Some(CLAMPED));
}

#[test]
fn unreadable_after_a_clamped_sample_keeps_the_frame_as_evidence() {
    let start = Instant::now();
    let mut samples = [
        (0, Ok(Some(BEFORE))),
        (100, Ok(Some(CLAMPED))),
        (101, Err(temporarily_unreadable())),
        (1000, Err(temporarily_unreadable())),
        (1099, Err(temporarily_unreadable())),
        (1100, Err(temporarily_unreadable())),
    ]
    .into_iter();
    let error = wait_for_geometry_with(
        MOVE,
        Some(BEFORE),
        Deadline::detached_after(2_000).unwrap(),
        || {
            let (millis, sample) = samples.next().expect("unexpected extra geometry read");
            (sample, start + Duration::from_millis(millis))
        },
    )
    .unwrap_err();
    assert!(
        samples.next().is_none(),
        "completed before the unreadable budget elapsed"
    );
    assert_unavailable(error, Some(CLAMPED));
}

#[test]
fn display_bounds_preserve_inventory_order_and_are_omitted_if_empty() {
    let mut details = json!({ "cause": "readback_mismatch" });
    add_display_bounds(&mut details, &[]);
    assert_eq!(details, json!({ "cause": "readback_mismatch" }));

    add_display_bounds(&mut details, &[BEFORE, CLAMPED]);
    assert_eq!(
        details,
        json!({
            "cause": "readback_mismatch",
            "display_bounds": [BEFORE, CLAMPED],
        })
    );
}

#[test]
fn maximize_readback_still_requires_the_requested_frame() {
    let error = wait_for_requested_geometry(MOVE, Deadline::detached_after(450).unwrap(), || {
        Ok(Some(CLAMPED))
    })
    .unwrap_err();
    assert_timeout(error);
}

#[test]
fn never_readable_geometry_reports_readback_unavailable_after_the_fallback_budget() {
    for sample in [Ok(None), Err(temporarily_unreadable())] {
        let error = wait_for_geometry_with(
            MOVE,
            Some(BEFORE),
            Deadline::detached_after(2_000).unwrap(),
            || (sample.clone(), Instant::now()),
        )
        .unwrap_err();
        assert_unavailable(error, None);
    }
}

#[test]
fn never_readable_geometry_times_out_if_the_deadline_expires_first() {
    let error = wait_for_geometry_with(
        MOVE,
        Some(BEFORE),
        Deadline::detached_after(20).unwrap(),
        || (Ok(None), Instant::now()),
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
            (Ok(frame), start + Duration::from_millis(millis))
        },
    );
    assert!(samples.next().is_none(), "completed before the final read");
    result
}

fn assert_mismatch(error: AdapterError, observed: Rect) {
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.unwrap();
    assert_eq!(details["cause"], "readback_mismatch");
    assert_eq!(details["observed"], json!(observed));
}

fn assert_unavailable(error: AdapterError, observed: Option<Rect>) {
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(
        error.message,
        "The window could not be read back after the request, so its effect is unknown"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Run 'list-windows' to see the window as it is now before sending the request again")
    );
    assert_eq!(
        error.details.unwrap(),
        json!({
            "cause": "readback_unavailable",
            "requested": { "x": 350.0, "y": -200.0 },
            "observed": observed,
        })
    );
}

fn temporarily_unreadable() -> AdapterError {
    AdapterError::new(ErrorCode::AppUnresponsive, "AX read unavailable")
}

fn assert_timeout(error: AdapterError) {
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
