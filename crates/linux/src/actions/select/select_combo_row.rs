use std::time::Duration;

use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
};
use serde_json::json;

use super::select_pointer::{
    Pointer, after_input_sent, already_satisfied, reserve_close, with_detail,
};
use super::select_pointer_combo::popup_not_observed;
use super::select_row::click_row;
use super::select_search::{Search, search};
use super::select_verify::read_verification;
use super::{BusReader, children_of, handle_of, miss_error};
use crate::actions::click::{POLL_STEP, poll_until};
use crate::actions::context::Target;
use crate::system::menu::overview;
use crate::tree::AtspiElement;
use crate::tree::capabilities::action_read::{NodeView, read_node_view};
use crate::tree::live_read::live_offscreen::toolkit_of;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind};
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, LIST};
use crate::tree::vocabulary::state_bits::{EXPANDABLE, EXPANDED, SHOWING};

const POPUP_WAIT: Duration = Duration::from_millis(1500);
const CLOSE_WAIT: Duration = Duration::from_millis(500);

pub(super) fn owner(target: &Target<'_>, view: &NodeView) -> Option<AtspiElement> {
    let combo = if view.role() == Some(COMBO_BOX) {
        target.element.clone()
    } else {
        target.element.ancestry().combo_owner()?
    };
    (kind(&toolkit_of(target.bus, &combo, target.deadline)) == ToolkitKind::Qt).then_some(combo)
}

trait ComboRows {
    fn satisfied(&mut self) -> Option<bool>;
    fn expanded(&mut self, deadline: Deadline) -> Option<bool>;
    fn open(&mut self) -> Result<Vec<ActionStep>, AdapterError>;
    fn popup_hidden(&self, deadline: Deadline) -> Option<bool>;
    fn escape(&mut self) -> Option<()>;
    fn overview_open(&self) -> Option<bool>;
    fn close(&mut self, deadline: Deadline) -> (bool, Option<bool>) {
        match self.popup_hidden(deadline) {
            Some(false) => {}
            hidden => return (false, hidden),
        }
        if self.escape().is_none() {
            return (false, None);
        }
        (
            true,
            poll_until(
                deadline,
                CLOSE_WAIT,
                POLL_STEP,
                || self.popup_hidden(deadline),
                |hidden| *hidden == Some(true),
            ),
        )
    }
    fn choose(&mut self, deadline: Deadline) -> Result<ActionResult, AdapterError>;
}

struct BusCombo<'a> {
    target: &'a Target<'a>,
    combo: &'a AtspiElement,
    wanted: &'a str,
    pointer: &'a dyn Pointer,
}

impl ComboRows for BusCombo<'_> {
    fn satisfied(&mut self) -> Option<bool> {
        read_verification(self.target.bus, self.target, self.combo, self.wanted, None)
    }

    fn expanded(&mut self, deadline: Deadline) -> Option<bool> {
        read_node_view(self.target.bus, &handle_of(self.combo), deadline)
            .ok()
            .and_then(|view| observed_expansion(view.state(EXPANDED), view.state(EXPANDABLE)))
    }

    fn open(&mut self) -> Result<Vec<ActionStep>, AdapterError> {
        self.pointer.click(self.combo)
    }

    fn popup_hidden(&self, deadline: Deadline) -> Option<bool> {
        let children = children_of(self.target.bus, self.combo, Some(COMBO_BOX), deadline).ok()?;
        let mut found = false;
        for child in children {
            let view = read_node_view(self.target.bus, &handle_of(&child), deadline).ok()?;
            if view.role()? == LIST {
                found = true;
                if view.state(SHOWING)? {
                    return Some(false);
                }
            }
        }
        found.then_some(true)
    }

    fn escape(&mut self) -> Option<()> {
        self.pointer.escape().ok().map(|_| ())
    }

    fn overview_open(&self) -> Option<bool> {
        overview::overview_open()
    }

    fn choose(&mut self, deadline: Deadline) -> Result<ActionResult, AdapterError> {
        let target = Target {
            deadline,
            ..*self.target
        };
        let mut reader = BusReader {
            bus: self.target.bus,
            deadline,
            inspected_role: None,
        };
        match search(&mut reader, self.combo, self.wanted)? {
            Search::Found(hit) => click_row(&target, &hit, self.wanted, self.pointer),
            miss => Err(miss_error(&miss)),
        }
    }
}

fn observed_expansion(expanded: Option<bool>, expandable: Option<bool>) -> Option<bool> {
    match (expanded, expandable) {
        (Some(true), _) => Some(true),
        (Some(false), Some(true)) => Some(false),
        _ => None,
    }
}

fn choose(rows: &mut dyn ComboRows, deadline: Deadline) -> Result<ActionResult, AdapterError> {
    if rows.satisfied() == Some(true) {
        return Ok(already_satisfied());
    }
    match rows.expanded(deadline) {
        Some(true) => return rows.choose(deadline),
        None => {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Cannot establish whether the dropdown is expanded",
            )
            .with_details(json!({"kind": "expansion_state_unread"}))
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        Some(false) => {}
    }
    let popup_deadline = reserve_close(deadline, CLOSE_WAIT)?;
    let opened = rows.open()?;
    let expanded = poll_until(
        popup_deadline,
        POPUP_WAIT,
        POLL_STEP,
        || rows.expanded(popup_deadline),
        |expanded| *expanded == Some(true),
    );
    if expanded != Some(true) {
        let hidden = rows.popup_hidden(deadline);
        let overview = if hidden == Some(true) {
            rows.overview_open()
        } else {
            None
        };
        let error = popup_not_observed(overview);
        return Err(close_after_opening(rows, deadline, error, hidden));
    }
    let mut result = rows.choose(popup_deadline).map_err(|error| {
        let hidden = rows.popup_hidden(deadline);
        close_after_opening(rows, deadline, error, hidden)
    })?;
    let selected = std::mem::take(&mut result.steps);
    Ok(result.with_steps(opened.into_iter().chain(selected).collect()))
}

fn close_after_opening(
    rows: &mut dyn ComboRows,
    deadline: Deadline,
    error: AdapterError,
    hidden: Option<bool>,
) -> AdapterError {
    let (attempted, closed) = if hidden == Some(false) {
        rows.close(deadline)
    } else {
        (false, hidden)
    };
    let error = with_detail(error, "popup_close_attempted", json!(attempted));
    after_opener(with_detail(error, "popup_closed", json!(closed)))
}

fn after_opener(mut error: AdapterError) -> AdapterError {
    let details = error.details.get_or_insert_with(|| json!({}));
    details["combo_opener_clicked"] = true.into();
    details["physical_delivery_started"] = true.into();
    if let Some(clicks) = details
        .get("delivered_clicks")
        .and_then(serde_json::Value::as_u64)
    {
        details["delivered_clicks"] = clicks.saturating_add(1).into();
    }
    after_input_sent(error)
}

pub(super) fn select(
    target: &Target<'_>,
    combo: &AtspiElement,
    wanted: &str,
    pointer: &dyn Pointer,
) -> Result<ActionResult, AdapterError> {
    choose(
        &mut BusCombo {
            target,
            combo,
            wanted,
            pointer,
        },
        target.deadline,
    )
}

#[cfg(test)]
#[path = "select_combo_row_tests.rs"]
mod tests;
