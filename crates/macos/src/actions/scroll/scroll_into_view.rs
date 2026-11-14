#[cfg(target_os = "macos")]
mod imp {
    use super::super::scroll_plan::bar_can_move;
    use super::super::scroll_read;
    use super::super::scroll_reveal::{RevealArea, reveal_steps};
    use crate::actions::chain::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;
    use agent_desktop_core::{
        AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, InteractionPolicy, Rect,
    };
    use std::time::{Duration, Instant};

    const MAX_SCROLL_ANCESTORS: usize = 4;
    const MAX_ANCESTOR_DEPTH: usize = 32;
    const MOVEMENT_READS: usize = 3;
    const MOVEMENT_PAUSE: Duration = Duration::from_millis(20);

    pub fn scroll_into_view_impl(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        scroll_into_view_outcome(element, deadline).map(|_| ())
    }

    pub(crate) fn scroll_into_view_outcome(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        match scroll_to_verified(element, deadline)? {
            outcome @ (DeliveryOutcome::SatisfiedNoDelivery
            | DeliveryOutcome::DeliveredVerified) => Ok(outcome),
            DeliveryOutcome::NotDelivered | DeliveryOutcome::DeliveredWithoutEffect => {
                scroll_ancestors_until_visible(element, deadline)
            }
            DeliveryOutcome::DeliveredUnverified => Err(unverified_scroll_error()),
        }
    }

    /// Scrolls each scroll area that encloses the target, innermost first, until
    /// the target lies inside that area's frame or the area proves it cannot
    /// move further. An inner area scrolls the target into its own frame; the
    /// next area out then moves that inner area and the target together.
    fn scroll_ancestors_until_visible(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let mut delivery = crate::actions::DeliveryTracker::default();
        let mut last_refusal = None;
        let ancestors = scroll_ancestors(element, deadline)?;
        for area in &ancestors {
            match reveal_within(element, area, &mut delivery, deadline) {
                Ok(Some(refusal)) => last_refusal = Some(refusal),
                Ok(None) => {}
                Err(error) => return Err(delivery.annotate(error)),
            }
        }
        if !visible_through_ancestors(element, deadline)
            .map_err(|error| delivery.annotate(error))?
        {
            let error = last_refusal.unwrap_or_else(|| still_hidden_error(ancestors.len()));
            return Err(delivery.annotate(error));
        }
        Ok(if delivery.delivered_units() == 0 {
            DeliveryOutcome::SatisfiedNoDelivery
        } else {
            DeliveryOutcome::DeliveredVerified
        })
    }

    fn still_hidden_error(ancestor_count: usize) -> AdapterError {
        if ancestor_count == 0 {
            return AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Target is hidden and has no scroll area ancestor to scroll",
            );
        }
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Target remains clipped after its scroll areas moved as far as they can",
        )
        .with_details(serde_json::json!({ "scroll_ancestors": ancestor_count }))
        .with_suggestion("Take a fresh snapshot; the target may sit behind another surface.")
    }

    fn scroll_ancestors(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Vec<AXElement>, AdapterError> {
        let mut areas = Vec::new();
        let mut current = scroll_read::element(element, "AXParent", deadline)?;
        for _ in 0..MAX_ANCESTOR_DEPTH {
            let Some(node) = current else {
                break;
            };
            match scroll_read::string(&node, "AXRole", deadline)?.as_deref() {
                Some("AXWindow" | "AXApplication") => break,
                Some("AXScrollArea") => {
                    areas.push(node.clone());
                    if areas.len() == MAX_SCROLL_ANCESTORS {
                        break;
                    }
                }
                _ => {}
            }
            current = scroll_read::element(&node, "AXParent", deadline)?;
        }
        Ok(areas)
    }

    struct LiveRevealArea<'a> {
        element: &'a AXElement,
        area: &'a AXElement,
        deadline: Deadline,
    }

    impl RevealArea for LiveRevealArea<'_> {
        fn target_bounds(&mut self) -> Result<Option<Rect>, AdapterError> {
            element_bounds(self.element, self.deadline)
        }

        fn viewport(&mut self) -> Result<Option<Rect>, AdapterError> {
            Ok(crate::tree::element_bounds::clip_bounds(
                self.area,
                "AXScrollArea",
                element_bounds(self.area, self.deadline)?,
                crate::tree::locator_deadline::from_operation(self.deadline)?,
            ))
        }

        fn step(
            &mut self,
            direction: &Direction,
            target: Rect,
        ) -> Result<Option<bool>, AdapterError> {
            scroll_step(self.area, self.element, direction, target, self.deadline)
        }

        fn target_moved(&mut self, target: Rect) -> Result<bool, AdapterError> {
            target_moved(self.element, target, self.deadline)
        }
    }

    fn reveal_within(
        element: &AXElement,
        area: &AXElement,
        delivery: &mut crate::actions::DeliveryTracker,
        deadline: Deadline,
    ) -> Result<Option<AdapterError>, AdapterError> {
        let mut live = LiveRevealArea {
            element,
            area,
            deadline,
        };
        reveal_steps(&mut live, delivery, deadline)
    }

    /// One semantic scroll of `area` toward the target. `None` when the area
    /// declined without any delivery: its scroll bar already sits at that
    /// limit, or no mechanism for the direction is available. `Some(moved)`
    /// once a mechanism was delivered, with `moved` read from the target's
    /// own bounds rather than from the mechanism's return code.
    fn scroll_step(
        area: &AXElement,
        element: &AXElement,
        direction: &Direction,
        before: Rect,
        deadline: Deadline,
    ) -> Result<Option<bool>, AdapterError> {
        if bar_at_limit(area, direction, deadline)? {
            return Ok(None);
        }
        match super::super::ax_scroll(area, direction, 1, InteractionPolicy::headless(), deadline) {
            Ok(_) => {}
            Err(error) if declined_without_delivery(&error) => return Ok(None),
            Err(error) => return Err(error),
        }
        target_moved(element, before, deadline)
            .map(Some)
            .map_err(after_delivery)
    }

    pub(super) fn declined_without_delivery(error: &AdapterError) -> bool {
        error.code == ErrorCode::ActionNotSupported
            && matches!(
                error.disposition,
                DeliverySemantics::Unknown | DeliverySemantics::NotDelivered
            )
    }

    fn bar_at_limit(
        area: &AXElement,
        direction: &Direction,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        let attribute = match direction {
            Direction::Up | Direction::Down => "AXVerticalScrollBar",
            Direction::Left | Direction::Right => "AXHorizontalScrollBar",
        };
        let Some(bar) = scroll_read::element(area, attribute, deadline)? else {
            return Ok(false);
        };
        Ok(scroll_read::number(&bar, "AXValue", deadline)?
            .is_some_and(|value| !bar_can_move(value, direction)))
    }

    fn target_moved(
        element: &AXElement,
        before: Rect,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        for read in 0..MOVEMENT_READS {
            if scroll_effect_observed(Some(before), element_bounds(element, deadline)?) {
                return Ok(true);
            }
            if read + 1 < MOVEMENT_READS {
                let pause = deadline.remaining_slice(MOVEMENT_PAUSE)?;
                std::thread::sleep(pause.min(MOVEMENT_PAUSE));
            }
        }
        Ok(false)
    }

    pub(crate) fn scroll_to_verified(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        prepare(element, deadline)?;
        let before = element_bounds(element, deadline)?;
        if !crate::actions::ax_helpers::try_ax_action_or_err(
            element,
            "AXScrollToVisible",
            deadline,
        )? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let local_end = Instant::now() + Duration::from_millis(800);
        loop {
            if visible_through_ancestors(element, deadline).map_err(after_delivery)? {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
            if deadline.is_expired() {
                return Err(after_delivery(deadline.timeout_error().with_details(
                    serde_json::json!({
                        "verification": "scroll_visibility_not_observed",
                    }),
                )));
            }
            if Instant::now() >= local_end {
                return settled_scroll_outcome(before, element_bounds(element, deadline));
            }
            let pause = deadline
                .remaining_slice(Duration::from_millis(20))
                .map_err(after_delivery)?;
            std::thread::sleep(pause.min(Duration::from_millis(20)));
        }
    }

    pub(super) fn settled_scroll_outcome(
        before: Option<Rect>,
        after: Result<Option<Rect>, AdapterError>,
    ) -> Result<DeliveryOutcome, AdapterError> {
        if !scroll_effect_observed(before, after.map_err(after_delivery)?) {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        Err(unverified_scroll_error())
    }

    fn element_bounds(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            crate::tree::locator_deadline::from_operation(deadline)?,
        )
    }

    pub(crate) fn scroll_effect_observed(before: Option<Rect>, after: Option<Rect>) -> bool {
        match (before, after) {
            (Some(before), Some(after)) => before.bounds_hash() != after.bounds_hash(),
            (None, None) => false,
            _ => true,
        }
    }

    pub(super) fn unverified_scroll_error() -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "AXScrollToVisible completed but target visibility was not verified",
        )
        .with_disposition(DeliverySemantics::delivered_unverified())
        .with_suggestion("Inspect target visibility before deciding whether to scroll again.")
    }

    fn visible_through_ancestors(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        if !inside_window(element, deadline)? {
            return Ok(false);
        }
        Ok(
            crate::tree::hit_test::visible_bounds_ax_element(element, deadline)?
                .is_some_and(rect_has_area),
        )
    }

    /// A view hosted out of process, such as a settings pane, answers
    /// `AXWindow` with no value although its parent chain reaches a window, so
    /// that chain is the fallback evidence. An element whose chain reaches no
    /// window is detached, and its visibility is never claimed.
    fn inside_window(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
        if scroll_read::element(element, "AXWindow", deadline)?.is_some() {
            return Ok(true);
        }
        let mut current = scroll_read::element(element, "AXParent", deadline)?;
        for _ in 0..MAX_ANCESTOR_DEPTH {
            let Some(node) = current else {
                return Ok(false);
            };
            if scroll_read::string(&node, "AXRole", deadline)?.as_deref() == Some("AXWindow") {
                return Ok(true);
            }
            current = scroll_read::element(&node, "AXParent", deadline)?;
        }
        Ok(false)
    }

    pub(crate) fn rect_has_area(rect: Rect) -> bool {
        rect.x.is_finite()
            && rect.y.is_finite()
            && rect.width.is_finite()
            && rect.height.is_finite()
            && rect.width > 0.0
            && rect.height > 0.0
    }

    fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
        crate::tree::properties::attributes::set_messaging_timeout(element, deadline)
    }

    fn after_delivery(error: AdapterError) -> AdapterError {
        error.with_disposition(DeliverySemantics::delivered_unverified())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::{AdapterError, Deadline};

    pub fn scroll_into_view_impl(
        _element: &AXElement,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("scroll_into_view"))
    }
}

pub(crate) use imp::scroll_into_view_impl;

#[cfg(target_os = "macos")]
pub(crate) use imp::scroll_into_view_outcome;

#[cfg(all(test, target_os = "macos"))]
pub(crate) use imp::{rect_has_area, scroll_effect_observed};

#[cfg(all(test, target_os = "macos"))]
#[path = "scroll_into_view_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
mod evidence_tests {
    #[test]
    fn unverified_scroll_to_visible_is_not_safe_to_retry() {
        assert_eq!(
            super::imp::unverified_scroll_error().disposition,
            agent_desktop_core::DeliverySemantics::delivered_unverified()
        );
    }
}
