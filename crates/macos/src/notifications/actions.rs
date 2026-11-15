use agent_desktop_core::{
    ActionResult, AdapterError, Deadline, ErrorCode, InteractionPolicy, NotificationFilter,
    NotificationInfo, NotificationTarget,
};

use super::nc_session::{NcSession, close_session};

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

pub(crate) fn dismiss_notification(
    target: &NotificationTarget,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<NotificationInfo, AdapterError> {
    require_foreground_policy(policy)?;
    let session = NcSession::open(policy, deadline)?;
    let result = dismiss_impl(target, policy, session.pid(), deadline);
    close_session(session, result)
}

pub(crate) fn dismiss_all(
    app_filter: Option<&str>,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
    require_foreground_policy(policy)?;
    let session = NcSession::open(policy, deadline)?;
    let result = dismiss_all_impl(app_filter, policy, session.pid(), deadline);
    close_session(session, result)
}

pub(crate) fn notification_action(
    target: &NotificationTarget,
    action_name: &str,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    require_foreground_policy(policy)?;
    let session = NcSession::open(policy, deadline)?;
    let result = action_impl(target, action_name, session.pid(), deadline);
    close_session(session, result)
}

fn dismiss_impl(
    target: &NotificationTarget,
    policy: InteractionPolicy,
    pid: i32,
    deadline: Deadline,
) -> Result<NotificationInfo, AdapterError> {
    let filter = NotificationFilter::default();
    let entries = super::list::list_entries(&filter, pid, deadline)?;

    let entry = &entries[target.pick(entries.iter().map(|e| &e.info))?];

    let info = entry.info.clone();
    let matching_before = super::dismiss_verify::matching_count(&entries, entry);
    dismiss_entry(&LiveDismiss {
        entry,
        filter: &filter,
        matching_before,
        pid,
        policy,
        deadline,
    })?;
    Ok(info)
}

fn dismiss_all_impl(
    app_filter: Option<&str>,
    policy: InteractionPolicy,
    pid: i32,
    deadline: Deadline,
) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
    let filter = NotificationFilter::for_app(app_filter);
    let entries = super::list::list_entries(&filter, pid, deadline)?;

    let mut dismissed = Vec::new();
    let mut failures = Vec::new();
    let mut delivery = crate::actions::DeliveryTracker::default();

    for original in entries.iter().rev() {
        let current_entries = super::list::list_entries(&filter, pid, deadline)
            .map_err(|error| delivery.annotate(error))?;
        let Some(current) = current_entries
            .iter()
            .find(|entry| super::dismiss_verify::matches(original, entry))
        else {
            failures.push(format!(
                "#{}: notification disappeared before dismissal was attempted",
                original.info.index
            ));
            continue;
        };
        let matching_before = super::dismiss_verify::matching_count(&current_entries, current);
        let outcome = dismiss_entry(&LiveDismiss {
            entry: current,
            filter: &filter,
            matching_before,
            pid,
            policy,
            deadline,
        });
        super::dismiss_strategy::record(&mut delivery, outcome.as_ref().map(|()| true));
        match outcome {
            Ok(()) => dismissed.push(original.info.clone()),
            Err(e) => failures.push(format!("#{}: {}", original.info.index, e)),
        }
    }

    Ok((dismissed, failures))
}

/// One notification to dismiss, with what verifying its removal needs.
struct LiveDismiss<'a> {
    entry: &'a super::list::NotificationEntry,
    filter: &'a NotificationFilter,
    matching_before: usize,
    pid: i32,
    policy: InteractionPolicy,
    deadline: Deadline,
}

fn dismiss_entry(target: &LiveDismiss<'_>) -> Result<(), AdapterError> {
    super::dismiss_strategy::run_dismiss(target, target.deadline)
}

impl super::dismiss_strategy::DismissOps for LiveDismiss<'_> {
    fn perform(&self, action: &str) -> Result<bool, AdapterError> {
        crate::actions::ax_helpers::try_ax_action_or_err(&self.entry.element, action, self.deadline)
    }

    fn press_close_button(
        &self,
        delivery: &mut crate::actions::DeliveryTracker,
    ) -> Result<bool, AdapterError> {
        let children = strategy_read(
            crate::notifications::read::children(&self.entry.element, self.deadline),
            self.deadline,
        )?
        .unwrap_or_default();
        try_dismiss_button(&children, delivery, self.deadline)
    }

    fn is_gone(&self) -> Result<bool, AdapterError> {
        super::dismiss_verify::disappeared(
            self.entry,
            self.filter,
            self.matching_before,
            self.pid,
            self.deadline,
        )
    }

    fn may_reveal(&self) -> Result<(), AdapterError> {
        if !crate::system::permissions::preflight_accessibility() {
            return Err(AdapterError::permission_denied());
        }
        if !self.policy.allow_cursor_move {
            return Err(AdapterError::policy_denied_for_policy(
                "Notification dismissal requires revealing its close control with the pointer",
                self.policy,
            ));
        }
        Ok(())
    }

    fn hover(&self) -> Result<(), AdapterError> {
        hover_over(&self.entry.element, self.deadline)
    }
}

fn try_dismiss_button(
    children: &[crate::tree::AXElement],
    delivery: &mut crate::actions::DeliveryTracker,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    for child in children {
        let role = strategy_read(
            crate::notifications::read::string(child, "AXRole", deadline),
            deadline,
        )?;
        let Some(role) = role else {
            continue;
        };
        if role.as_deref() != Some("AXButton") {
            continue;
        }
        let name = strategy_read(
            crate::notifications::read::title_or_description(child, deadline),
            deadline,
        )?
        .flatten()
        .unwrap_or_default()
        .to_lowercase();
        let is_dismiss =
            name.contains("close") || name.contains("clear") || name.contains("dismiss");
        if !is_dismiss {
            continue;
        }
        let pressed = crate::actions::ax_helpers::try_ax_action_or_err(child, "AXPress", deadline);
        super::dismiss_strategy::record(delivery, pressed.as_ref().copied());
        if strategy_succeeded(pressed, deadline)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Presses a named action button on the notification core picks from a
/// fresh read of the Notification Center.
fn action_impl(
    target: &NotificationTarget,
    action_name: &str,
    pid: i32,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let filter = NotificationFilter::default();
    let entries = super::list::list_entries(&filter, pid, deadline)?;

    let entry = &entries[target.pick(entries.iter().map(|e| &e.info))?];
    let index = entry.info.index;

    let children = crate::notifications::read::children(&entry.element, deadline)?;
    let action_lower = action_name.to_lowercase();
    let mut action_btn = None;
    for child in &children {
        let role = crate::notifications::read::string(child, "AXRole", deadline)?;
        let identifier = crate::notifications::read::string(child, "AXIdentifier", deadline)?;
        let name =
            crate::notifications::read::title_or_description(child, deadline)?.unwrap_or_default();
        if role.as_deref() == Some("AXButton")
            && crate::notifications::scan::is_notification_action(identifier.as_deref())
            && name.to_lowercase() == action_lower
        {
            action_btn = Some(child);
            break;
        }
    }
    let btn = action_btn.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "Action '{}' not found on notification {}",
                action_name, index
            ),
        )
    })?;

    if !crate::actions::ax_helpers::try_ax_action_or_err(btn, "AXPress", deadline)? {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "Failed to press '{}' button on notification {}",
                action_name, index
            ),
        ));
    }

    Ok(ActionResult::delivered_unverified(action_name))
}

fn hover_over(el: &crate::tree::AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    use agent_desktop_core::{MouseButton, MouseEvent, MouseEventKind, Point};

    let bounds = crate::notifications::read::bounds(el, deadline)?
        .ok_or_else(|| AdapterError::internal("Cannot read notification bounds for hover"))?;

    crate::input::mouse::synthesize_mouse(
        MouseEvent {
            kind: MouseEventKind::Move,
            point: Point {
                x: bounds.x + bounds.width / 2.0,
                y: bounds.y + bounds.height / 2.0,
            },
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        deadline,
    )
}

fn require_foreground_policy(policy: InteractionPolicy) -> Result<(), AdapterError> {
    if policy.allow_focus_steal {
        Ok(())
    } else {
        Err(AdapterError::policy_denied_for_policy(
            "Notification Center interaction requires foreground access",
            policy,
        ))
    }
}

fn strategy_succeeded(
    outcome: Result<bool, AdapterError>,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    match outcome {
        Ok(succeeded) => Ok(succeeded),
        Err(error) => {
            crate::notifications::read::tolerate_ax_strategy_error(error, deadline)?;
            Ok(false)
        }
    }
}

fn strategy_read<T>(
    outcome: Result<T, AdapterError>,
    deadline: Deadline,
) -> Result<Option<T>, AdapterError> {
    match outcome {
        Ok(value) => Ok(Some(value)),
        Err(error) => {
            crate::notifications::read::tolerate_ax_strategy_error(error, deadline)?;
            Ok(None)
        }
    }
}
