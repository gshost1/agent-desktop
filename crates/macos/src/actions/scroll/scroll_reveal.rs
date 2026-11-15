//! The step loop that reveals a target inside one scroll area.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, Rect};

use super::scroll_plan::{BlockedDirections, next_direction, next_reveal_direction};
use crate::actions::DeliveryTracker;

const MAX_STEPS_PER_ANCESTOR: usize = 24;

/// What the reveal loop reads from and does to one scroll area.
pub(super) trait RevealArea {
    fn target_bounds(&mut self) -> Result<Option<Rect>, AdapterError>;
    fn viewport(&mut self) -> Result<Option<Rect>, AdapterError>;
    /// One scroll toward the target: `None` when the area declined without
    /// delivery, `Some(moved)` once a mechanism was delivered.
    fn step(&mut self, direction: &Direction, target: Rect) -> Result<Option<bool>, AdapterError>;
    fn target_moved(&mut self, target: Rect) -> Result<bool, AdapterError>;
}

/// Scrolls `area` toward the target until the plan settles. A step whose
/// mechanism failed after it may have been delivered does not end the
/// reveal: the target's own bounds decide whether that direction still
/// moves, and the failure is returned only so the caller can report it if
/// the target never becomes visible. A reveal that stops because the next
/// step would reverse the last one returns an overshoot error instead, since
/// the area could still move. The postcondition, not an intermediate return
/// code, decides the outcome.
pub(super) fn reveal_steps(
    area: &mut impl RevealArea,
    delivery: &mut DeliveryTracker,
    deadline: Deadline,
) -> Result<Option<AdapterError>, AdapterError> {
    let mut blocked = BlockedDirections::default();
    let mut previous = None;
    let mut refusal = None;
    for _ in 0..MAX_STEPS_PER_ANCESTOR {
        deadline.ensure_remaining()?;
        let target = area
            .target_bounds()?
            .ok_or_else(|| AdapterError::new(ErrorCode::ActionFailed, "Target has no bounds"))?;
        let Some(viewport) = area.viewport()? else {
            break;
        };
        let Some(direction) = next_reveal_direction(target, viewport, blocked, previous.as_ref())
        else {
            if next_direction(target, viewport, blocked).is_some() {
                refusal = Some(overshoot_error());
            }
            break;
        };
        let moved = match area.step(&direction, target) {
            Ok(Some(moved)) => {
                delivery.mark_delivered();
                moved
            }
            Ok(None) => false,
            Err(error) if is_step_refusal(&error) => {
                if error.disposition != DeliverySemantics::NotDelivered {
                    delivery.mark_delivered();
                }
                refusal = Some(error);
                area.target_moved(target).map_err(after_delivery)?
            }
            Err(error) => return Err(error),
        };
        if moved {
            previous = Some(direction);
        } else {
            blocked.block(&direction);
        }
    }
    Ok(refusal)
}

fn is_step_refusal(error: &AdapterError) -> bool {
    !matches!(error.code, ErrorCode::Timeout | ErrorCode::PermDenied)
}

fn overshoot_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Target remains clipped: each page-sized scroll step moved it past the viewport",
    )
    .with_details(serde_json::json!({ "kind": "scroll_step_overshoot" }))
    .with_suggestion(
        "The area can still move, but its page steps are larger than the gap to the target. \
         Scroll it with '--headed', whose wheel steps are smaller, then retry.",
    )
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "scroll_reveal_tests.rs"]
mod tests;
