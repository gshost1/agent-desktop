use super::*;
use crate::actions::ax_helpers;
use crate::tree::{AXElement, surface_read};
use std::time::Instant;

type Neighbour = (AXElement, String, String);

pub(super) fn end_edit(field: &AXElement, deadline: Deadline) -> EndedEdit {
    let Ok(instant) = crate::tree::locator_deadline::from_operation(deadline) else {
        return EndedEdit::NotAttempted;
    };
    if focused(field, deadline) != Some(true) {
        return EndedEdit::NotAttempted;
    }
    let ancestors = ancestors_to_window(field, instant);
    let neighbours = ancestors.first().map_or_else(Vec::new, |parent| {
        neighbour_values(parent, field, instant, deadline)
    });
    let window = ancestors
        .last()
        .filter(|e| role(e, deadline).as_deref() == Some("AXWindow"));
    let mut candidates = vec![("focus_off", (field.clone(), "AXFocused"))];
    if let Some(window) = window {
        candidates.push(("container_focus", (window.clone(), "AXMain")));
        candidates.push(("container_focus", (window.clone(), "AXFocused")));
        let nodes = bounded_nodes(window, instant);
        if let Some(nodes) = nodes {
            for element in &nodes {
                if matches!(
                    role(element, deadline).as_deref(),
                    Some("AXGroup" | "AXScrollArea" | "AXStaticText")
                ) && safe_container(element, instant, deadline)
                {
                    candidates.push(("container_focus", (element.clone(), "AXFocused")));
                }
            }
            for element in nodes {
                if role(&element, deadline).as_deref() == Some("AXTextArea")
                    && safe_text_area(action_names(&element, deadline).as_deref())
                {
                    candidates.push(("text_area_focus", (element, "AXFocused")));
                }
            }
        }
    }
    candidates.push(("confirm", (field.clone(), "AXConfirm")));
    let (ended, _) = ordered_attempts(
        candidates
            .into_iter()
            .take_while(|_| !deadline.remaining().is_zero()),
        |mechanism, (target, attribute)| {
            if deadline.remaining().is_zero() {
                return Trial::Continue;
            }
            let succeeded = if mechanism == "confirm" {
                ax_helpers::try_ax_action_or_err(field, "AXConfirm", deadline)
            } else {
                ax_helpers::set_ax_bool_or_err(
                    &target,
                    attribute,
                    mechanism != "focus_off",
                    deadline,
                )
            }
            .unwrap_or(false);
            if !succeeded {
                return Trial::Continue;
            }
            if mechanism == "container_focus" && !safe_app_focus(field, instant, deadline) {
                let _ = ax_helpers::ax_focus_or_err(field, deadline);
                return Trial::Delegated;
            }
            let mut changes = read_changes(&neighbours, deadline);
            if mechanism == "confirm" {
                for _ in 0..5 {
                    if confirm_evidence(focused(field, deadline), !changes.is_empty())
                        || deadline.remaining().is_zero()
                    {
                        break;
                    }
                    std::thread::sleep(
                        deadline
                            .remaining()
                            .min(std::time::Duration::from_millis(20)),
                    );
                    changes = read_changes(&neighbours, deadline);
                }
            }
            let ended = if mechanism == "confirm" {
                confirm_evidence(focused(field, deadline), !changes.is_empty())
            } else {
                focused(field, deadline) == Some(false)
            };
            if ended {
                return Trial::Ended(changes);
            }
            if has_sheet(window, instant) {
                return Trial::Refused;
            }
            Trial::Continue
        },
    );
    match ended {
        EndedEdit::Committed(mut evidence) => {
            let returned = ax_helpers::ax_focus_or_err(field, deadline).ok() == Some(true)
                && focused(field, deadline) == Some(true);
            evidence["focus_returned"] = json!(returned);
            evidence["dependent_changes"] = json!(read_changes(&neighbours, deadline));
            if returned {
                EndedEdit::Committed(evidence)
            } else {
                EndedEdit::NotRestored(evidence)
            }
        }
        other => other,
    }
}

fn has_sheet(window: Option<&AXElement>, instant: Instant) -> bool {
    window.is_some_and(|w| {
        surface_read::elements(w, "AXSheets", instant).is_ok_and(|v| !v.is_empty())
    })
}

fn role(element: &AXElement, deadline: Deadline) -> Option<String> {
    string_read(element, "AXRole", deadline).flatten()
}

fn action_names(element: &AXElement, deadline: Deadline) -> Option<Vec<String>> {
    let mut usage = crate::tree::observation_usage::ObservationUsage::with_defaults();
    crate::tree::properties::capabilities::copy_action_names_with_status(
        element,
        Instant::now() + deadline.remaining(),
        &mut usage,
    )
    .value
}

fn safe_app_focus(field: &AXElement, instant: Instant, deadline: Deadline) -> bool {
    let Some(app) = application(field, instant) else {
        return false;
    };
    let Some(focus) = surface_read::element(&app, "AXFocusedUIElement", instant)
        .ok()
        .flatten()
    else {
        return false;
    };
    if crate::tree::same_element(&focus, field) {
        return true;
    }
    safe_focused_role(
        role(&focus, deadline).as_deref(),
        action_names(&focus, deadline).as_deref(),
    )
}

fn application(field: &AXElement, instant: Instant) -> Option<AXElement> {
    let mut current = field.clone();
    for _ in 0..36 {
        if surface_read::string(&current, "AXRole", instant)
            .ok()
            .flatten()
            .as_deref()
            == Some("AXApplication")
        {
            return Some(current);
        }
        current = surface_read::element(&current, "AXParent", instant)
            .ok()
            .flatten()?;
    }
    None
}

fn bounded_nodes(root: &AXElement, instant: Instant) -> Option<Vec<AXElement>> {
    let mut nodes = Vec::new();
    let mut pending =
        std::collections::VecDeque::from(surface_read::elements(root, "AXChildren", instant).ok()?);
    while let Some(node) = pending.pop_front() {
        if nodes.len() >= MAX_PEER_NODES {
            return None;
        }
        pending.extend(surface_read::elements(&node, "AXChildren", instant).ok()?);
        nodes.push(node);
    }
    Some(nodes)
}

fn safe_container(element: &AXElement, instant: Instant, deadline: Deadline) -> bool {
    bounded_nodes(element, instant).is_some_and(|nodes| {
        nodes.iter().all(|node| {
            safe_subtree_node(
                role(node, deadline).as_deref(),
                ax_helpers::is_attr_settable(node, "AXFocused", deadline).ok(),
            )
        })
    })
}

fn read_changes(neighbours: &[Neighbour], deadline: Deadline) -> Vec<Value> {
    let before = neighbours
        .iter()
        .map(|(_, role, value)| (role.clone(), value.clone()))
        .collect::<Vec<_>>();
    let after = neighbours
        .iter()
        .map(|(element, _, _)| crate::tree::copy_value_typed(element, deadline))
        .collect::<Vec<_>>();
    changed_values(&before, &after)
}

fn focused(element: &AXElement, deadline: Deadline) -> Option<bool> {
    let read_deadline = crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline).ok()?;
    crate::tree::properties::attributes::copy_bool_attr_result(element, "AXFocused", read_deadline)
        .ok()
        .flatten()
}

fn string_read(element: &AXElement, attribute: &str, deadline: Deadline) -> Option<Option<String>> {
    let read_deadline = crate::tree::properties::ax_ipc::AxDeadline::absolute(deadline).ok()?;
    crate::tree::properties::attributes::copy_string_attr_result(element, attribute, read_deadline)
        .ok()
}

fn ancestors_to_window(field: &AXElement, instant: Instant) -> Vec<AXElement> {
    let mut chain = Vec::new();
    let mut current = surface_read::element(field, "AXParent", instant)
        .ok()
        .flatten();
    while let Some(element) = current {
        let role = surface_read::string(&element, "AXRole", instant)
            .ok()
            .flatten();
        if matches!(role.as_deref(), Some("AXApplication") | None) {
            break;
        }
        let is_window = role.as_deref() == Some("AXWindow");
        current = (!is_window)
            .then(|| {
                surface_read::element(&element, "AXParent", instant)
                    .ok()
                    .flatten()
            })
            .flatten();
        chain.push(element);
        if is_window || chain.len() > 32 {
            break;
        }
    }
    chain
}

fn neighbour_values(
    parent: &AXElement,
    field: &AXElement,
    instant: Instant,
    deadline: Deadline,
) -> Vec<Neighbour> {
    let mut values = Vec::new();
    let mut pending = std::collections::VecDeque::from(
        surface_read::elements(parent, "AXChildren", instant).unwrap_or_default(),
    );
    let mut visited = 0;
    while let Some(element) = pending.pop_front() {
        visited += 1;
        if values.len() >= MAX_NEIGHBOUR_VALUES || visited > MAX_PEER_NODES {
            break;
        }
        if crate::tree::same_element(&element, field) {
            continue;
        }
        if let Some(Some(role)) = string_read(&element, "AXRole", deadline) {
            let subrole = string_read(&element, "AXSubrole", deadline);
            if reports_value(&role, subrole.as_ref().map(|subrole| subrole.as_deref()))
                && let Some(value) = crate::tree::copy_value_typed(&element, deadline)
            {
                values.push((
                    element.clone(),
                    crate::tree::roles::ax_role_to_str(&role).to_string(),
                    value,
                ));
            }
        }
        pending.extend(surface_read::elements(&element, "AXChildren", instant).unwrap_or_default());
    }
    values
}
