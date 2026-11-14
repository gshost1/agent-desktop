use crate::{AccessibilityNode, snapshot::diff::diff_key::IdentityKey, tree::state::FOCUSED};

/// Everything a diff compares about one row: its identity and every field a
/// reader sees. Two rows with equal content read the same, which is the only
/// claim a diff makes about identical twins it cannot tell apart.
#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct RowContent<'a> {
    key: IdentityKey<'a>,
    name: Option<&'a str>,
    value: Option<&'a str>,
    placeholder: Option<&'a str>,
    hint: Option<&'a str>,
    states: Vec<&'a str>,
}

impl<'a> RowContent<'a> {
    pub(crate) fn of(node: &'a AccessibilityNode) -> Self {
        let mut states: Vec<&str> = node
            .presentation
            .states
            .iter()
            .map(String::as_str)
            .collect();
        states.sort_unstable();
        Self {
            key: IdentityKey::of(node),
            name: node.identity.name.as_deref(),
            value: node.identity.value.as_deref(),
            placeholder: node.presentation.descriptors.placeholder.as_deref(),
            hint: node.presentation.hint.as_deref(),
            states,
        }
    }
}

/// The fields a reader sees differ between `before` and `after`, in a fixed
/// order: `name`, `value`, `description`, `placeholder`, `hint`, then
/// `states` for any state other than focus and `focus` for keyboard focus.
pub(crate) fn changed_fields(
    before: &AccessibilityNode,
    after: &AccessibilityNode,
) -> Vec<&'static str> {
    let texts = [
        ("name", &before.identity.name, &after.identity.name),
        ("value", &before.identity.value, &after.identity.value),
        (
            "description",
            &before.identity.description,
            &after.identity.description,
        ),
        (
            "placeholder",
            &before.presentation.descriptors.placeholder,
            &after.presentation.descriptors.placeholder,
        ),
        ("hint", &before.presentation.hint, &after.presentation.hint),
    ];
    let mut changed: Vec<&'static str> = texts
        .into_iter()
        .filter(|(_, old, new)| old != new)
        .map(|(field, _, _)| field)
        .collect();
    if other_states(before) != other_states(after) {
        changed.push("states");
    }
    if is_focused(before) != is_focused(after) {
        changed.push("focus");
    }
    changed
}

fn other_states(node: &AccessibilityNode) -> Vec<&str> {
    let mut states: Vec<&str> = node
        .presentation
        .states
        .iter()
        .map(String::as_str)
        .filter(|state| *state != FOCUSED)
        .collect();
    states.sort_unstable();
    states
}

fn is_focused(node: &AccessibilityNode) -> bool {
    node.presentation.states.iter().any(|state| state == FOCUSED)
}

#[cfg(test)]
#[path = "row_content_tests.rs"]
mod tests;
