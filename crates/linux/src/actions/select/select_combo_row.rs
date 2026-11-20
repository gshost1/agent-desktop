use agent_desktop_core::{
    ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
};
use serde_json::json;

use super::select_pointer::{Pointer, after_input_sent, already_satisfied};
use super::select_row::click_row;
use super::select_search::{Search, search};
use super::select_verify::read_verification;
use super::{BusReader, handle_of, miss_error};
use crate::actions::click::{POLL_STEP, poll_until};
use crate::actions::context::Target;
use crate::tree::AtspiElement;
use crate::tree::capabilities::action_read::{NodeView, read_node_view};
use crate::tree::live_read::live_offscreen::toolkit_of;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind};
use crate::tree::vocabulary::role_numbers::COMBO_BOX;
use crate::tree::vocabulary::state_bits::EXPANDED;

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
    fn expanded(&mut self) -> Option<bool>;
    fn open(&mut self) -> Result<Vec<ActionStep>, AdapterError>;
    fn choose(&mut self) -> Result<ActionResult, AdapterError>;
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

    fn expanded(&mut self) -> Option<bool> {
        read_node_view(
            self.target.bus,
            &handle_of(self.combo),
            self.target.deadline,
        )
        .ok()
        .and_then(|view| view.state(EXPANDED))
    }

    fn open(&mut self) -> Result<Vec<ActionStep>, AdapterError> {
        self.pointer.click(self.combo)
    }

    fn choose(&mut self) -> Result<ActionResult, AdapterError> {
        let mut reader = BusReader {
            bus: self.target.bus,
            deadline: self.target.deadline,
            inspected_role: None,
        };
        match search(&mut reader, self.combo, self.wanted)? {
            Search::Found(hit) => click_row(self.target, &hit, self.wanted, self.pointer),
            miss => Err(miss_error(&miss)),
        }
    }
}

fn choose(rows: &mut dyn ComboRows, deadline: Deadline) -> Result<ActionResult, AdapterError> {
    if rows.satisfied() == Some(true) {
        return Ok(already_satisfied());
    }
    match rows.expanded() {
        Some(true) => return rows.choose(),
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
    let opened = rows.open()?;
    let expanded = poll_until(
        deadline,
        std::time::Duration::from_millis(1500),
        POLL_STEP,
        || rows.expanded(),
        |expanded| *expanded == Some(true),
    );
    if expanded != Some(true) {
        return Err(after_opener(
            AdapterError::new(
                ErrorCode::ActionFailed,
                "The dropdown did not show its choices after the pointer click",
            )
            .with_details(json!({"kind": "popup_not_observed"}))
            .with_disposition(DeliverySemantics::delivered_unverified()),
        ));
    }
    let mut result = rows.choose().map_err(after_opener)?;
    let selected = std::mem::take(&mut result.steps);
    Ok(result.with_steps(opened.into_iter().chain(selected).collect()))
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
