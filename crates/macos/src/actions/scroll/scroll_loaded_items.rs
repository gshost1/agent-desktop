//! The items a scroll area has loaded, below its wrappers and beside its
//! headers.
//!
//! A table or outline sits in its scroll area beside a column header, and
//! lists its column objects after its rows, so neither the only child nor
//! the last child is reliably an item. The container is the one collection
//! among the area's content, and an item of a table or outline is one of its
//! rows.

use agent_desktop_core::{AdapterError, Deadline, Direction};

use crate::tree::AXElement;

const MAX_WRAPPER_DEPTH: usize = 3;
const MAX_WRAPPER_CHILDREN: usize = 8;
const MAX_ITEMS: usize = 4096;
const FARTHEST_SCAN: usize = 16;
pub(super) const COLLECTION_ROLES: &[&str] = &["AXOutline", "AXTable", "AXList", "AXGrid"];
const ROW_CONTAINERS: &[&str] = &["AXOutline", "AXTable"];
const NEVER_ITEMS: &[&str] = &["AXColumn", "AXScrollBar"];

/// The children of the area's item container, read up to a bound that is
/// not an error: a list that loaded more than the bound still has its
/// farthest read item loaded, so revealing it skips nothing.
pub(super) struct LoadedItems {
    items: Vec<AXElement>,
    container_role: Option<String>,
}

impl LoadedItems {
    pub(super) fn read(area: &AXElement, deadline: Deadline) -> Result<Self, AdapterError> {
        let mut level = super::scroll_read::children_up_to(area, MAX_ITEMS, deadline)?;
        let mut container_role = None;
        for _ in 0..MAX_WRAPPER_DEPTH {
            if level.len() > MAX_WRAPPER_CHILDREN {
                break;
            }
            let roles = roles_of(&level, deadline)?;
            let Some(index) = container_among(&roles) else {
                break;
            };
            container_role = roles[index].clone();
            let container = level.swap_remove(index);
            level = super::scroll_read::children_up_to(&container, MAX_ITEMS, deadline)?;
        }
        Ok(Self {
            items: level,
            container_role,
        })
    }

    /// The loaded item farthest toward `direction`, skipping column objects,
    /// scroll bars and, in a table or outline, anything that is not a row.
    pub(super) fn farthest(
        &self,
        direction: &Direction,
        deadline: Deadline,
    ) -> Result<Option<AXElement>, AdapterError> {
        let toward_end = matches!(direction, Direction::Down | Direction::Right);
        let candidates: Vec<&AXElement> = if toward_end {
            self.items.iter().rev().take(FARTHEST_SCAN).collect()
        } else {
            self.items.iter().take(FARTHEST_SCAN).collect()
        };
        for candidate in candidates {
            let role = super::scroll_read::string(candidate, "AXRole", deadline)?;
            if is_item(self.container_role.as_deref(), role.as_deref()) {
                return Ok(Some(candidate.clone()));
            }
        }
        Ok(None)
    }

    pub(super) fn middle(&self) -> Option<&AXElement> {
        self.items.get(self.items.len() / 2)
    }
}

fn roles_of(
    elements: &[AXElement],
    deadline: Deadline,
) -> Result<Vec<Option<String>>, AdapterError> {
    elements
        .iter()
        .map(|element| super::scroll_read::string(element, "AXRole", deadline))
        .collect()
}

/// Which child holds the items: the only child that is not a scroll bar, or
/// else the only collection among them. `None` when the children are the
/// items themselves.
pub(super) fn container_among(roles: &[Option<String>]) -> Option<usize> {
    let content: Vec<usize> = (0..roles.len())
        .filter(|index| roles[*index].as_deref() != Some("AXScrollBar"))
        .collect();
    if let [only] = content.as_slice() {
        return Some(*only);
    }
    let mut collections = content.into_iter().filter(|index| {
        roles[*index]
            .as_deref()
            .is_some_and(|role| COLLECTION_ROLES.contains(&role))
    });
    let first = collections.next()?;
    collections.next().is_none().then_some(first)
}

/// Whether a child of the container is one of its items.
pub(super) fn is_item(container_role: Option<&str>, role: Option<&str>) -> bool {
    if role.is_some_and(|role| NEVER_ITEMS.contains(&role)) {
        return false;
    }
    if container_role.is_some_and(|container| ROW_CONTAINERS.contains(&container)) {
        return role == Some("AXRow");
    }
    true
}

#[cfg(test)]
#[path = "scroll_loaded_items_tests.rs"]
mod tests;
