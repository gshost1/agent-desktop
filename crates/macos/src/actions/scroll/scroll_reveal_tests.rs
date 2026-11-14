use std::collections::VecDeque;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, Rect};

use super::{RevealArea, reveal_steps};
use crate::actions::DeliveryTracker;

fn rect(y: f64) -> Rect {
    Rect {
        x: 10.0,
        y,
        width: 100.0,
        height: 20.0,
    }
}

struct ScriptedArea {
    target_y: f64,
    steps: VecDeque<(f64, Result<Option<bool>, AdapterError>)>,
    taken: Vec<String>,
}

impl RevealArea for ScriptedArea {
    fn target_bounds(&mut self) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(rect(self.target_y)))
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
        let (moved_to, result) = self.steps.pop_front().unwrap_or((self.target_y, Ok(None)));
        self.target_y = moved_to;
        result
    }

    fn target_moved(&mut self, target: Rect) -> Result<bool, AdapterError> {
        Ok(target.y != self.target_y)
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
    let mut area = ScriptedArea {
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
}

#[test]
fn a_revealed_target_reports_nothing() {
    let (result, area, delivered) = reveal(900.0, vec![(100.0, Ok(Some(true)))]);

    assert!(result.unwrap().is_none());
    assert_eq!(area.taken, ["Down"]);
    assert_eq!(delivered, 1);
}
