use agent_desktop_core::{
    Action, ActionRequest, ActionResult, AdapterError, Deadline, DeliverySemantics, Direction,
    ErrorCode, InteractionLease, KeyCombo, PlatformAdapter,
};
use serde_json::json;

use super::browser_link::BrowserLink;
use super::web_channel_adapter::WebChannelAdapter;
use super::web_handle::WebHandle;
use crate::{
    ElementToken, PageSession, append_text, click_times, fill, focus, press_key, read_element,
    scroll_by, scroll_into_view, select_option, set_checked,
};

const SCROLL_STEP_PX: f64 = 40.0;

/// One action on one DOM node, with the page session it runs in.
struct WebTarget<'a> {
    link: &'a mut BrowserLink,
    session: PageSession,
    token: &'a ElementToken,
    deadline: Deadline,
}

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    /// Runs an action on a web element over CDP. Every mapping works on a
    /// background tab: none brings the page to the front, activates its
    /// window or moves the OS cursor.
    pub(crate) fn execute_web_action(
        &self,
        handle: &WebHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if let Some(expected) = request.expected_process()
            && expected != &handle.process
        {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The web element belongs to a different browser process than the request names",
            )
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        let deadline = lease.deadline();
        let token = &handle.token;
        let result = self.with_required_link(&handle.process, deadline, |link| {
            let session = link.session(&token.target_id, deadline)?;
            let mut target = WebTarget {
                link,
                session,
                token,
                deadline,
            };
            let result = run(&mut target, &request.action);
            if result.is_ok() {
                let _ = self.note_downloads(link, may_start_download(&request.action), deadline);
            }
            result
        })?;
        Ok(with_channel(result))
    }
}

fn run(target: &mut WebTarget<'_>, action: &Action) -> Result<ActionResult, AdapterError> {
    let name = action.name();
    match action {
        Action::Click => target
            .click(1)
            .map(|()| ActionResult::delivered_unverified(name)),
        Action::DoubleClick => target
            .click(2)
            .map(|()| ActionResult::delivered_unverified(name)),
        Action::SetValue(text) => target.fill(name, text),
        Action::Clear => target.fill(name, ""),
        Action::TypeText(text) => target.append(name, text),
        Action::Select(choice) => target.select(name, choice),
        Action::Check => target.set_checked(name, true),
        Action::Uncheck => target.set_checked(name, false),
        Action::Toggle => target.toggle(name),
        Action::SetFocus => {
            focus(
                &mut target.link.connection,
                &target.session,
                target.token,
                target.deadline,
            )?;
            Ok(ActionResult::delivered_unverified(name))
        }
        Action::ScrollTo => {
            scroll_into_view(
                &mut target.link.connection,
                &target.session,
                target.token,
                target.deadline,
            )?;
            Ok(ActionResult::delivered_unverified(name))
        }
        Action::Scroll(direction, amount) => target.scroll(name, direction.clone(), *amount),
        Action::PressKey(combo) => target.press(name, combo),
        _ => Err(unsupported(name)),
    }
}

impl WebTarget<'_> {
    fn click(&mut self, count: u32) -> Result<(), AdapterError> {
        click_times(
            &mut self.link.connection,
            &self.session,
            self.token,
            count,
            self.deadline,
        )
        .map(|_| ())
    }

    fn fill(&mut self, name: &str, text: &str) -> Result<ActionResult, AdapterError> {
        let outcome = fill(
            &mut self.link.connection,
            &self.session,
            self.token,
            text,
            self.deadline,
        )?;
        let result = verdict(name, outcome.verified);
        Ok(if outcome.verified {
            result
        } else {
            result.with_details(json!({ "observed": outcome.observed }))
        })
    }

    fn append(&mut self, name: &str, text: &str) -> Result<ActionResult, AdapterError> {
        append_text(
            &mut self.link.connection,
            &self.session,
            self.token,
            text,
            self.deadline,
        )?;
        let value = self.read()?.value;
        Ok(verdict(
            name,
            value.is_some_and(|value| value.ends_with(text)),
        ))
    }

    fn select(&mut self, name: &str, choice: &str) -> Result<ActionResult, AdapterError> {
        let selected = select_option(
            &mut self.link.connection,
            &self.session,
            self.token,
            choice,
            self.deadline,
        )?;
        let value = self.read()?.value;
        Ok(verdict(name, value.as_deref() == Some(selected.as_str())))
    }

    fn set_checked(&mut self, name: &str, checked: bool) -> Result<ActionResult, AdapterError> {
        let Some(current) = self.read()?.checked else {
            return Err(not_checkable(name));
        };
        if current == checked {
            return Ok(ActionResult::satisfied_without_delivery(name));
        }
        set_checked(
            &mut self.link.connection,
            &self.session,
            self.token,
            checked,
            self.deadline,
        )?;
        Ok(verdict(name, true))
    }

    fn toggle(&mut self, name: &str) -> Result<ActionResult, AdapterError> {
        let Some(current) = self.read()?.checked else {
            return Err(not_checkable(name));
        };
        set_checked(
            &mut self.link.connection,
            &self.session,
            self.token,
            !current,
            self.deadline,
        )?;
        Ok(verdict(name, true))
    }

    fn scroll(
        &mut self,
        name: &str,
        direction: Direction,
        amount: u32,
    ) -> Result<ActionResult, AdapterError> {
        let step = SCROLL_STEP_PX * f64::from(amount.max(1));
        let delta = match direction {
            Direction::Up => (0.0, -step),
            Direction::Down => (0.0, step),
            Direction::Left => (-step, 0.0),
            Direction::Right => (step, 0.0),
        };
        let moved = scroll_by(
            &mut self.link.connection,
            &self.session,
            self.token,
            delta,
            self.deadline,
        )?;
        let result = verdict(name, moved);
        Ok(if moved {
            result
        } else {
            result.with_details(json!({ "moved": false }))
        })
    }

    fn press(&mut self, name: &str, combo: &KeyCombo) -> Result<ActionResult, AdapterError> {
        if !combo.modifiers.is_empty() {
            return Err(unsupported(name).with_suggestion(
                "The web channel presses single keys; type text with 'type' or press the key without modifiers",
            ));
        }
        focus(
            &mut self.link.connection,
            &self.session,
            self.token,
            self.deadline,
        )?;
        press_key(
            &mut self.link.connection,
            &self.session,
            &combo.key,
            self.deadline,
        )?;
        Ok(ActionResult::delivered_unverified(name))
    }

    fn read(&mut self) -> Result<crate::ElementReading, AdapterError> {
        read_element(
            &mut self.link.connection,
            &self.session,
            self.token,
            self.deadline,
        )
    }
}

/// Actions that can follow a link or submit a form, and so start a download.
fn may_start_download(action: &Action) -> bool {
    matches!(
        action,
        Action::Click | Action::DoubleClick | Action::PressKey(_)
    )
}

fn verdict(name: &str, verified: bool) -> ActionResult {
    let result = ActionResult::delivered_unverified(name);
    if verified {
        result.with_verified_delivery()
    } else {
        result
    }
}

fn with_channel(mut result: ActionResult) -> ActionResult {
    let details = result.details.get_or_insert_with(|| json!({}));
    if let Some(details) = details.as_object_mut() {
        details.insert("channel".into(), "cdp".into());
    }
    result
}

fn not_checkable(name: &str) -> AdapterError {
    unsupported(name).with_suggestion(
        "Check, uncheck and toggle work on checkboxes, radio buttons and switches; use click for other controls",
    )
}

fn unsupported(name: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("'{name}' is not available on a web element over CDP"),
    )
    .with_suggestion("Use click, set-value, type, select, check, uncheck, toggle, focus, scroll, scroll-to or press on web refs")
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "web_action_tests.rs"]
mod tests;
