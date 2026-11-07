use agent_desktop_core::actions::capability;

use super::cdp_ax_node::CdpAxNode;

/// The core capabilities this crate can deliver to a node of `role`.
pub(crate) fn actions_of(role: &str, node: &CdpAxNode) -> Vec<String> {
    let flag = |name: &str| node.property(name).is_some_and(|value| value.is_true());
    let editable = node
        .property("editable")
        .and_then(|value| value.as_text())
        .is_some_and(|value| value != "false");
    let mut actions: Vec<&str> = Vec::new();
    match role {
        "button" | "link" | "menuitem" | "menubutton" | "tab" | "option" | "treeitem"
        | "disclosure" | "cell" => actions.push(capability::CLICK),
        "checkbox" | "radiobutton" | "switch" => {
            actions.extend([capability::CLICK, capability::TOGGLE]);
        }
        "combobox" if editable => {
            actions.extend([
                capability::CLICK,
                capability::SET_VALUE,
                capability::TYPE_TEXT,
            ]);
        }
        "combobox" => actions.extend([capability::CLICK, capability::SELECT]),
        "textfield" => actions.extend([capability::SET_VALUE, capability::TYPE_TEXT]),
        _ if editable => actions.extend([capability::SET_VALUE, capability::TYPE_TEXT]),
        _ => {}
    }
    if flag("focusable") {
        actions.push(capability::SET_FOCUS);
    }
    actions.into_iter().map(str::to_owned).collect()
}

#[cfg(test)]
#[path = "node_actions_tests.rs"]
mod tests;
