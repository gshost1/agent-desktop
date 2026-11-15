use std::cmp::Ordering;

use agent_desktop_core::{Direction, Rect};

/// Scroll directions an ancestor has shown it cannot move in while one target
/// is being revealed: its scroll bar sits at that limit, or every semantic
/// mechanism for that direction declined or left the target where it was.
/// A viewport frame can disagree with what the user sees, so a direction the
/// geometry asks for is still dropped once the ancestor proves it cannot move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BlockedDirections {
    mask: u8,
}

impl BlockedDirections {
    pub(crate) fn block(&mut self, direction: &Direction) {
        self.mask |= direction_bit(direction);
    }

    pub(crate) fn is_blocked(self, direction: &Direction) -> bool {
        self.mask & direction_bit(direction) != 0
    }
}

fn direction_bit(direction: &Direction) -> u8 {
    match direction {
        Direction::Up => 1,
        Direction::Down => 2,
        Direction::Left => 4,
        Direction::Right => 8,
    }
}

/// Chooses the next scroll that moves `target` toward `viewport`, from the
/// observed geometry alone. Each axis is judged on its own: an axis needs a
/// scroll only when the target crosses exactly one of the viewport's edges on
/// it, so a target larger than the viewport on an axis never asks for one. An
/// axis on which the target lies entirely outside the viewport goes before an
/// axis on which it is only clipped, because that axis alone hides it; on a
/// tie the vertical axis goes first. Blocked directions are never chosen.
pub(crate) fn next_direction(
    target: Rect,
    viewport: Rect,
    blocked: BlockedDirections,
) -> Option<Direction> {
    let vertical = axis_need(
        (target.y, target.y + target.height),
        (viewport.y, viewport.y + viewport.height),
    )
    .map(|(side, detached)| (toward(side, Direction::Up, Direction::Down), detached))
    .filter(|(direction, _)| !blocked.is_blocked(direction));
    let horizontal = axis_need(
        (target.x, target.x + target.width),
        (viewport.x, viewport.x + viewport.width),
    )
    .map(|(side, detached)| (toward(side, Direction::Left, Direction::Right), detached))
    .filter(|(direction, _)| !blocked.is_blocked(direction));
    match (vertical, horizontal) {
        (Some((_, false)), Some((direction, true))) => Some(direction),
        (Some((direction, _)), _) | (None, Some((direction, _))) => Some(direction),
        (None, None) => None,
    }
}

/// The next scroll while revealing a target inside one area, given the
/// direction of the step this area last took. A direction that reverses the
/// last step means page-sized steps are overshooting the target, so the area
/// stops instead of bouncing between the two. A target that is only partly
/// inside the viewport keeps scrolling the way it was going: the visible part
/// can be a sliver under the area's own scroll bar, where no pointer lands.
pub(crate) fn next_reveal_direction(
    target: Rect,
    viewport: Rect,
    blocked: BlockedDirections,
    previous: Option<&Direction>,
) -> Option<Direction> {
    let direction = next_direction(target, viewport, blocked)?;
    if previous.is_some_and(|previous| reverses(previous, &direction)) {
        return None;
    }
    Some(direction)
}

fn reverses(previous: &Direction, next: &Direction) -> bool {
    matches!(
        (previous, next),
        (Direction::Up, Direction::Down)
            | (Direction::Down, Direction::Up)
            | (Direction::Left, Direction::Right)
            | (Direction::Right, Direction::Left)
    )
}

fn toward(side: Ordering, before: Direction, after: Direction) -> Direction {
    if side == Ordering::Less {
        before
    } else {
        after
    }
}

fn axis_need(span: (f64, f64), view: (f64, f64)) -> Option<(Ordering, bool)> {
    let (start, end) = span;
    let (view_start, view_end) = view;
    let before = start < view_start;
    let after = end > view_end;
    if before == after {
        return None;
    }
    let detached = end <= view_start || start >= view_end;
    let side = if before {
        Ordering::Less
    } else {
        Ordering::Greater
    };
    Some((side, detached))
}

/// Whether a scroll bar whose `AXValue` is `value` can still move content in
/// `direction`. The value is the fraction scrolled, 0 at the start and 1 at
/// the end, so a bar at 0 cannot scroll up or left and a bar at 1 cannot
/// scroll down or right.
pub(crate) fn bar_can_move(value: f64, direction: &Direction) -> bool {
    const EDGE: f64 = 0.000_1;
    if !value.is_finite() {
        return true;
    }
    match direction {
        Direction::Up | Direction::Left => value > EDGE,
        Direction::Down | Direction::Right => value < 1.0 - EDGE,
    }
}

#[cfg(test)]
#[path = "scroll_plan_tests.rs"]
mod tests;
