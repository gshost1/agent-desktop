//! The capability tokens a snapshot withholds because the application's
//! toolkit refuses their headless route. A snapshot's `available_actions`
//! promise a headless semantic action, so a token whose only route the action
//! path refuses before writing is not printed. Live evidence keeps the token:
//! the action's preflight still needs it to reach a route that an explicit
//! `--headed` request opens, such as the row click of a headed select.

use agent_desktop_core::{LocatorEvidence, LocatorField, actions::capability};

use super::action_names::{names_select, unsupported_entry_activation};
use super::capability_rules::{Facts, select_rests_on_indices};
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::properties::toolkit_trust::selection_indices_are_accessible;
use crate::tree::{ObservedRead, RawNode};

/// The toolkit a snapshot read for the application it observed, keyed by
/// that application's bus name. A node of another application, or a
/// snapshot whose toolkit read did not answer, keeps every token.
pub(crate) struct HeadlessRoutes {
    application: Option<(String, ToolkitId)>,
}

impl HeadlessRoutes {
    pub(crate) fn unread() -> Self {
        Self { application: None }
    }

    pub(crate) fn of_application(bus_name: &str, toolkit: ToolkitId) -> Self {
        Self {
            application: Some((bus_name.to_owned(), toolkit)),
        }
    }

    pub(crate) fn withhold(&self, read: &ObservedRead, evidence: &mut LocatorEvidence) {
        let Some((bus_name, toolkit)) = &self.application else {
            return;
        };
        if read.node.element.bus_name() != bus_name {
            return;
        }
        let withheld = withheld_tokens(&read.node, toolkit);
        if let LocatorField::Known(tokens) = &mut evidence.ref_evidence.available_actions {
            tokens.retain(|token| !withheld.contains(&token.as_str()));
        }
    }
}

/// The tokens of `node` whose every route the action path refuses headless on
/// `toolkit`: `Click` of a GTK 4 entry whose only click verb is `activate`
/// (L41-22), and `Select` that rests on `Selection` indices the toolkit does
/// not map to accessible children (L40-2, L41-12) with no select verb of the
/// node's own.
pub(crate) fn withheld_tokens(node: &RawNode, toolkit: &ToolkitId) -> Vec<&'static str> {
    let Some(facts) = Facts::of(node) else {
        return Vec::new();
    };
    let mut withheld = Vec::new();
    if unsupported_entry_activation(Some(facts.role), facts.names, toolkit) {
        withheld.push(capability::CLICK);
    }
    if !selection_indices_are_accessible(toolkit)
        && select_rests_on_indices(&facts)
        && !names_select(&facts)
    {
        withheld.push(capability::SELECT);
    }
    withheld
}

#[cfg(test)]
#[path = "headless_routes_tests.rs"]
mod tests;
