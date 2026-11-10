//! One element of the open-menu search, read in one accessibility round trip.
//!
//! The search walks the whole application looking for a displayed menu, and
//! it needs only an element's role, its visibility and its children. Read one
//! attribute at a time, that cost a role read, a child count, a child page and
//! a second count per element: about 700 to 950 round trips after one click
//! in a SwiftUI application. One batched read answers all three. The
//! application builds the child list inside that single answer, so the list
//! cannot change between a count and a read the way it can across separate
//! calls, and no second count is needed to prove it.

use agent_desktop_core::{AdapterError, ErrorCode};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    string::CFString,
};
use std::time::Instant;

use super::AXElement;

const ATTRIBUTES: [&str; 3] = ["AXRole", "AXVisible", "AXChildren"];
const ROLE: usize = 0;
const VISIBLE: usize = 1;
const CHILDREN: usize = 2;

/// What the search learns from one element: whether it is a displayed menu,
/// and the children to search next.
pub(crate) struct MenuSearchNode {
    pub(crate) is_menu: bool,
    pub(crate) children: Vec<AXElement>,
}

pub(crate) fn attribute_names() -> CFArray<CFString> {
    CFArray::from_CFTypes(&ATTRIBUTES.map(CFString::new))
}

/// Reads one element. A menu bar is not searched below, a menu that does not
/// say it is hidden is the answer, and anything else hands back its children.
/// A busy application, an element that went away and a child list longer than
/// one observation reads fail the way the attribute-at-a-time reads failed.
pub(crate) fn read(
    element: &AXElement,
    names: &CFArray<CFString>,
    deadline: Instant,
) -> Result<MenuSearchNode, AdapterError> {
    super::surface_read::ensure_before_deadline(deadline)?;
    let (error, values) = super::read_recovery::read_owned(deadline, || {
        super::properties::ax_ipc::copy_multiple_attribute_values(
            element,
            names.as_concrete_TypeRef(),
            deadline,
        )
    });
    let values = super::properties::cf_type::created_cf_array(values);
    if error == accessibility_sys::kAXErrorAPIDisabled {
        return Err(super::surface_read::map_error("AXChildren", error));
    }
    super::surface_read::ensure_before_deadline(deadline)?;
    answer(error, values)
}

/// An element that answers the whole batch with an absence code, as AppKit's
/// synthetic elements do, has no role and no children to search, the same
/// answer three separate reads gave it.
fn answer(error: i32, values: Option<CFArray<CFType>>) -> Result<MenuSearchNode, AdapterError> {
    if super::properties::ax_absence::is_absent_attribute_error(error) {
        return Ok(leaf(false));
    }
    if error != accessibility_sys::kAXErrorSuccess {
        return Err(super::surface_read::map_error("AXChildren", error));
    }
    let values = values
        .filter(|values| values.len() as usize == ATTRIBUTES.len())
        .ok_or_else(|| {
            super::surface_read::map_error("AXChildren", accessibility_sys::kAXErrorFailure)
        })?;
    let slots: Vec<Option<CFType>> = values
        .iter()
        .map(|item| present_slot(&item))
        .collect::<Result<_, _>>()?;
    decode(&slots)
}

fn decode(slots: &[Option<CFType>]) -> Result<MenuSearchNode, AdapterError> {
    let slot = |index: usize| slots.get(index).and_then(Option::as_ref);
    let role = slot(ROLE).and_then(|item| item.downcast::<CFString>().map(|role| role.to_string()));
    let visible = slot(VISIBLE)
        .and_then(|item| item.downcast::<CFBoolean>())
        .map(bool::from);
    match role.as_deref() {
        Some("AXMenuBar") => Ok(leaf(false)),
        Some("AXMenu") if visible != Some(false) => Ok(leaf(true)),
        _ => Ok(MenuSearchNode {
            is_menu: false,
            children: slot(CHILDREN)
                .map(children)
                .transpose()?
                .unwrap_or_default(),
        }),
    }
}

fn leaf(is_menu: bool) -> MenuSearchNode {
    MenuSearchNode {
        is_menu,
        children: Vec::new(),
    }
}

fn children(item: &CFType) -> Result<Vec<AXElement>, AdapterError> {
    if item.type_of() != unsafe { core_foundation_sys::array::CFArrayGetTypeID() } {
        return Ok(Vec::new());
    }
    let array = unsafe { CFArray::<CFType>::wrap_under_get_rule(item.as_CFTypeRef() as _) };
    let limit = agent_desktop_core::ObservationBudget::default().max_children_per_node;
    let total = array.len() as usize;
    if total > limit {
        return Err(incomplete_children(total, limit));
    }
    let elements: Vec<AXElement> = array
        .iter()
        .filter_map(|value| super::properties::ax_value::retained_ax_element(&value))
        .collect();
    if elements.len() != total {
        return Err(super::surface_read::map_error(
            "AXChildren",
            accessibility_sys::kAXErrorFailure,
        ));
    }
    Ok(elements)
}

fn present_slot(item: &CFType) -> Result<Option<CFType>, AdapterError> {
    use super::properties::node_attribute_decode::{is_null, slot_error};
    if is_null(item) {
        return Ok(None);
    }
    match slot_error(item) {
        Some(error)
            if error == accessibility_sys::kAXErrorCannotComplete
                || error == accessibility_sys::kAXErrorInvalidUIElement =>
        {
            Err(super::surface_read::map_error("AXChildren", error))
        }
        Some(_) => Ok(None),
        None => Ok(Some(item.clone())),
    }
}

fn incomplete_children(total: usize, limit: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface read was incomplete for AXChildren",
    )
    .with_details(serde_json::json!({
        "kind": "surface_array_incomplete",
        "retryable": false,
        "attribute": "AXChildren",
        "complete": false,
        "total_count": total,
        "loaded_count": limit,
        "count_changed": false,
    }))
}

#[cfg(test)]
#[path = "menu_search_node_tests.rs"]
mod tests;
