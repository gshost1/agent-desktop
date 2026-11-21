use std::collections::VecDeque;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, Rect};

use super::{RevealArea, reveal_steps};
use crate::actions::DeliveryTracker;

fn rect(x: f64, y: f64) -> Rect {
    Rect {
        x,
        y,
        width: 100.0,
        height: 20.0,
    }
}

struct ScriptedArea {
    target_x: f64,
    target_y: f64,
    steps: VecDeque<(f64, Result<Option<bool>, AdapterError>)>,
    taken: Vec<String>,
}

impl RevealArea for ScriptedArea {
    fn target_bounds(&mut self) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(rect(self.target_x, self.target_y)))
    }

    fn viewport(&mut self) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        }))
    }

    fn step(&mut self, direction: &Direction, _: Rect) -> Result<Option<bool>, AdapterError> {
        self.taken.push(format!("{direction:?}"));
        let axis = match direction {
            Direction::Up | Direction::Down => &mut self.target_y,
            Direction::Left | Direction::Right => &mut self.target_x,
        };
        let (moved_to, result) = self.steps.pop_front().unwrap_or((*axis, Ok(None)));
        *axis = moved_to;
        result
    }

    fn target_moved(&mut self, target: Rect) -> Result<bool, AdapterError> {
        Ok(target.x != self.target_x || target.y != self.target_y)
    }
}

fn reveal(
    target_y: f64,
    steps: Vec<(f64, Result<Option<bool>, AdapterError>)>,
) -> (
    Result<Option<AdapterError>, AdapterError>,
    ScriptedArea,
    usize,
) {
    reveal_from(10.0, target_y, steps)
}

fn reveal_from(
    target_x: f64,
    target_y: f64,
    steps: Vec<(f64, Result<Option<bool>, AdapterError>)>,
) -> (
    Result<Option<AdapterError>, AdapterError>,
    ScriptedArea,
    usize,
) {
    let mut area = ScriptedArea {
        target_x,
        target_y,
        steps: steps.into(),
        taken: Vec::new(),
    };
    let mut delivery = DeliveryTracker::default();
    let result = reveal_steps(&mut area, &mut delivery, Deadline::after(5_000).unwrap());
    (result, area, delivery.delivered_units())
}

fn failed_after_delivery() -> AdapterError {
    AdapterError::new(ErrorCode::AppUnresponsive, "page step returned late")
        .with_disposition(DeliverySemantics::delivered_unverified())
}

#[test]
fn a_failed_step_does_not_end_the_reveal() {
    let (result, area, delivered) = reveal(
        900.0,
        vec![
            (600.0, Err(failed_after_delivery())),
            (100.0, Ok(Some(true))),
        ],
    );

    assert_eq!(
        area.taken,
        ["Down", "Down"],
        "a step that failed after it may have been delivered must not stop the reveal; the \
         target's bounds showed the direction still moves"
    );
    assert_eq!(area.target_y, 100.0);
    assert_eq!(delivered, 2);
    let refusal = result.unwrap().expect("the failure is kept for the caller");
    assert_eq!(refusal.code, ErrorCode::AppUnresponsive);
}

#[test]
fn a_failed_step_that_left_the_target_in_place_blocks_only_that_direction() {
    let (result, area, _) = reveal(900.0, vec![(900.0, Err(failed_after_delivery()))]);

    assert_eq!(area.taken, ["Down"]);
    assert!(result.unwrap().is_some());
}

#[test]
fn a_step_that_was_not_delivered_marks_no_delivery_and_the_other_axis_still_scrolls() {
    let refused = AdapterError::new(ErrorCode::ActionNotSupported, "page action refused")
        .with_disposition(DeliverySemantics::not_delivered());
    let (result, area, delivered) = reveal_from(
        500.0,
        900.0,
        vec![(900.0, Err(refused)), (100.0, Ok(Some(true)))],
    );

    assert_eq!(
        area.taken,
        ["Down", "Right"],
        "a refused step blocks only its own direction; the reveal goes on along the other axis"
    );
    assert_eq!(area.target_x, 100.0);
    assert_eq!(
        delivered, 1,
        "only the step that moved the target was delivered; the refused one never reached the \
         area"
    );
    let refusal = result.unwrap().expect("the refusal is kept for the caller");
    assert_eq!(refusal.code, ErrorCode::ActionNotSupported);
}

#[test]
fn a_timed_out_step_ends_the_reveal() {
    let (result, area, _) = reveal(
        900.0,
        vec![(600.0, Err(AdapterError::timeout("step timed out")))],
    );

    assert_eq!(result.unwrap_err().code, ErrorCode::Timeout);
    assert_eq!(area.taken, ["Down"]);
}

#[test]
fn a_reveal_stopped_by_overshooting_says_so() {
    let (result, area, _) = reveal(320.0, vec![(-40.0, Ok(Some(true)))]);

    assert_eq!(area.taken, ["Down"]);
    let error = result.unwrap().expect("an overshoot is reported");
    assert_eq!(error.details.unwrap()["kind"], "scroll_step_overshoot");
    assert!(
        !error.message.contains("as far as they can"),
        "the area could still move; only its page steps were too large"
    );
    assert!(
        error.message.contains("the last scroll step that moved it")
            && error.message.contains("past the viewport edge")
            && !error.message.contains("each"),
        "only the last step crossed the viewport, and it need not have been a page: {}",
        error.message
    );
}

#[test]
fn a_revealed_target_reports_nothing() {
    let (result, area, delivered) = reveal(900.0, vec![(100.0, Ok(Some(true)))]);

    assert!(result.unwrap().is_none());
    assert_eq!(area.taken, ["Down"]);
    assert_eq!(delivered, 1);
}
