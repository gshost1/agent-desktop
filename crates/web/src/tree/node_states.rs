use agent_desktop_core::tree::state;

use super::cdp_ax_node::CdpAxNode;

/// Reads the core state tokens a CDP node's properties prove. A state is
/// granted only on a positive claim; an absent property grants nothing.
pub(crate) fn states_of(node: &CdpAxNode) -> Vec<String> {
    let mut states = Vec::new();
    let flag = |name: &str| node.property(name).is_some_and(|value| value.is_true());
    let token = |name: &str| node.property(name).and_then(|value| value.as_text());
    let mut grant = |condition: bool, token: &str| {
        if condition {
            states.push(token.to_owned());
        }
    };
    grant(flag("focused"), state::FOCUSED);
    grant(flag("disabled"), state::DISABLED);
    grant(flag("expanded"), state::EXPANDED);
    grant(token("checked").as_deref() == Some("true"), state::CHECKED);
    grant(
        token("checked").as_deref() == Some("mixed"),
        state::INDETERMINATE,
    );
    grant(flag("selected"), state::SELECTED);
    grant(flag("hidden"), state::HIDDEN);
    grant(flag("busy"), state::BUSY);
    grant(flag("modal"), state::MODAL);
    grant(flag("required"), state::REQUIRED);
    grant(
        matches!(token("pressed").as_deref(), Some("true" | "mixed")),
        state::PRESSED,
    );
    grant(flag("readonly"), state::READONLY);
    grant(
        token("invalid").is_some_and(|value| value != "false"),
        state::INVALID,
    );
    grant(flag("multiselectable"), state::MULTISELECTABLE);
    grant(
        token("hasPopup").is_some_and(|value| value != "false"),
        state::HASPOPUP,
    );
    states
}

#[cfg(test)]
#[path = "node_states_tests.rs"]
mod tests;
