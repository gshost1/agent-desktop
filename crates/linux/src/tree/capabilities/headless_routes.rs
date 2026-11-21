//! The capability tokens a snapshot withholds because the application's
//! toolkit refuses their headless route. A snapshot's `available_actions`
//! promise a headless semantic action, so a token whose every route the
//! action path refuses before writing is not printed. A snapshot that could
//! not read the toolkit keeps its tokens until a live read can judge them.
//! A node read for an
//! action is projected without this pass: its live evidence keeps `Select`,
//! which the preflight of a `--headed select` needs to reach the row click,
//! and a `--headed click` passes preflight without a `Click` token.

use agent_desktop_core::{LocatorEvidence, LocatorField, actions::capability};

use super::action_names::{click_verbs, names_select, unsupported_entry_activation};
use super::capability_rules::{Facts, holds_selection_interface, select_rests_on_indices};
use crate::system::process::toolkit_id::ToolkitId;
use crate::tree::properties::toolkit_trust::{ToolkitKind, kind, selection_indices_are_accessible};
use crate::tree::vocabulary::role_numbers::COMBO_BOX;
use crate::tree::{ObservedRead, Walk, WalkedNode};

/// The toolkit a snapshot read for the application it observed, keyed by
/// that application's bus name. A node of another application, a toolkit
/// that refuses none of these routes, or a live read keeps every token.
pub(crate) struct HeadlessRoutes {
    application: Option<(String, ToolkitId)>,
}

fn may_refuse(toolkit: &ToolkitId) -> bool {
    matches!(kind(toolkit), ToolkitKind::Gtk4 | ToolkitKind::Qt)
        || !selection_indices_are_accessible(toolkit)
}

impl HeadlessRoutes {
    /// The routes of a node read for an action, which keeps every token.
    pub(crate) fn every_token() -> Self {
        Self { application: None }
    }

    pub(crate) fn of_application(bus_name: &str, toolkit: ToolkitId) -> Self {
        Self {
            application: may_refuse(&toolkit).then(|| (bus_name.to_owned(), toolkit)),
        }
    }

    /// The routes of a walk whose placement read `toolkit` from the walked
    /// application's root. A read that did not answer, or was not made,
    /// keeps every token. A read that answered without naming a toolkit
    /// withholds the index selects that the action path refuses for it.
    pub(crate) fn of_walk(walk: &Walk<ObservedRead>, toolkit: Option<ToolkitId>) -> Self {
        match toolkit {
            Some(toolkit) => {
                Self::of_application(walk.root.payload.node.element.bus_name(), toolkit)
            }
            None => Self::every_token(),
        }
    }

    pub(crate) fn withhold(&self, node: &WalkedNode<ObservedRead>, evidence: &mut LocatorEvidence) {
        let Some((bus_name, toolkit)) = &self.application else {
            return;
        };
        if node.payload.node.element.bus_name() != bus_name {
            return;
        }
        let withheld = withheld_tokens(node, toolkit);
        if let LocatorField::Known(tokens) = &mut evidence.ref_evidence.available_actions {
            tokens.retain(|token| !withheld.contains(&token.as_str()));
        }
    }
}

enum OwnRoute {
    Offered,
    Absent,
    Unread,
}

fn holds_selection(node: &WalkedNode<ObservedRead>) -> bool {
    Facts::of(&node.payload.node).is_some_and(|facts| holds_selection_interface(&facts))
}

fn own_route(node: &ObservedRead, row: bool) -> OwnRoute {
    let Some(facts) = Facts::of(&node.node) else {
        return OwnRoute::Unread;
    };
    let clicks = click_verbs(Some(facts.role));
    let clickable = row
        && facts
            .names
            .iter()
            .any(|name| clicks.iter().any(|verb| name.eq_ignore_ascii_case(verb)));
    match names_select(&facts) || clickable {
        true => OwnRoute::Offered,
        false => OwnRoute::Absent,
    }
}

/// Whether a select on this node can still land through the action of an
/// item below it that the search can match: an item that names a select
/// verb, or a row (a child of a `Selection` holder) that names a click verb,
/// the verb a select of a row falls back to. An item whose actions could not
/// be read might offer one.
fn an_item_offers_its_own_route(root: &WalkedNode<ObservedRead>) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        let rows = holds_selection(node);
        for child in &node.children {
            if !matches!(own_route(&child.payload, rows), OwnRoute::Absent) {
                return true;
            }
            pending.push(child);
        }
    }
    false
}

/// The tokens of `node` whose every headless route the action path refuses
/// on `toolkit`: `Click` of a GTK 4 entry whose only click verb is `activate`
/// (L41-22), and `Select` that rests on `Selection` indices the toolkit does
/// not map to accessible children (L40-2, L41-12), when neither the node nor
/// an item a select on it can match offers an action of its own. Qt combos
/// and their choices also withhold `Select`: their route always needs a pointer.
pub(crate) fn withheld_tokens(
    node: &WalkedNode<ObservedRead>,
    toolkit: &ToolkitId,
) -> Vec<&'static str> {
    let Some(facts) = Facts::of(&node.payload.node) else {
        return Vec::new();
    };
    let mut withheld = Vec::new();
    if unsupported_entry_activation(Some(facts.role), facts.names, toolkit) {
        withheld.push(capability::CLICK);
    }
    if kind(toolkit) == ToolkitKind::Qt
        && (facts.role == COMBO_BOX || node.payload.node.element.ancestry().combo_owner().is_some())
    {
        withheld.push(capability::SELECT);
    }
    if !selection_indices_are_accessible(toolkit)
        && select_rests_on_indices(&facts)
        && !names_select(&facts)
        && !an_item_offers_its_own_route(node)
    {
        withheld.push(capability::SELECT);
    }
    withheld
}

#[cfg(test)]
#[path = "headless_routes_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "headless_routes_combo_tests.rs"]
mod combo_tests;
