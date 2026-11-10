//! One element of the open-menu search, read in one accessibility round trip.
//!
//! The search walks the whole application looking for a displayed menu, and
//! it needs an element's role and children, and a menu's visibility. Read one
//! attribute at a time, that cost a role read, a child count, a child page and
//! a second count per element: about 700 to 950 round trips after one click
//! in a SwiftUI application. One batched read answers role and children. The
//! application builds the child list inside that single answer, so the list
//! cannot change between a count and a read the way it can across separate
//! calls, and no second count is needed to prove it. Visibility stays a
//! separate read made only for a menu: some applications compute it slowly,
//! and asking every element for it cost more than the round trips it saved.
//!
//! The batch is only a faster way to get a clean answer. When any value the
//! search needs comes back as an error, in an unexpected shape or as a child
//! list longer than one observation reads, the element is read again one
//! attribute at a time, so busy, vanished and oversized elements fail or are
//! skipped exactly as they were before.

use agent_desktop_core::AdapterError;
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    string::CFString,
};
use std::time::Instant;

use super::AXElement;

const ATTRIBUTES: [&str; 2] = ["AXRole", "AXChildren"];
const ROLE: usize = 0;
const CHILDREN: usize = 1;

/// What the search learns from one element: whether it is a displayed menu,
/// and the children to search next.
pub(crate) struct MenuSearchNode {
    pub(crate) is_menu: bool,
    pub(crate) children: Vec<AXElement>,
}

/// A clean batch answer: the role, and the child list unless it needs the
/// paged read to judge it.
struct Batch {
    role: Option<String>,
    children: Option<Vec<AXElement>>,
}

/// One answer slot of the batch.
enum Slot {
    Value(CFType),
    Absent,
    Unclear,
}

pub(crate) fn attribute_names() -> CFArray<CFString> {
    CFArray::from_CFTypes(&ATTRIBUTES.map(CFString::new))
}

/// Reads one element in one round trip. A menu bar is not searched below, a
/// menu that does not say it is hidden is the answer, and anything else hands
/// back its children. `None` means the batch gave no clean answer and the
/// caller reads the element one attribute at a time.
pub(crate) fn read(
    element: &AXElement,
    names: &CFArray<CFString>,
    deadline: Instant,
) -> Result<Option<MenuSearchNode>, AdapterError> {
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
        return Err(super::surface_read::map_error("AXRole", error));
    }
    super::surface_read::ensure_before_deadline(deadline)?;
    if error != accessibility_sys::kAXErrorSuccess {
        return Ok(None);
    }
    let Some(batch) = values
        .filter(|values| values.len() as usize == ATTRIBUTES.len())
        .and_then(|values| decode(&values.iter().map(|item| item.clone()).collect::<Vec<_>>()))
    else {
        return Ok(None);
    };
    let displayed_menu = batch.role.as_deref() == Some("AXMenu")
        && super::surface_read::boolean(element, "AXVisible", deadline)? != Some(false);
    Ok(search_node(batch, displayed_menu))
}

fn search_node(batch: Batch, displayed_menu: bool) -> Option<MenuSearchNode> {
    if batch.role.as_deref() == Some("AXMenuBar") {
        return Some(leaf(false));
    }
    if displayed_menu {
        return Some(leaf(true));
    }
    batch.children.map(|children| MenuSearchNode {
        is_menu: false,
        children,
    })
}

fn decode(values: &[CFType]) -> Option<Batch> {
    let role = match slot(values.get(ROLE)?, is_absent_property_error) {
        Slot::Value(item) => Some(item.downcast::<CFString>()?.to_string()),
        Slot::Absent => None,
        Slot::Unclear => return None,
    };
    let children = match slot(values.get(CHILDREN)?, is_absent_children_error) {
        Slot::Value(item) => children(&item),
        Slot::Absent => Some(Vec::new()),
        Slot::Unclear => None,
    };
    Some(Batch { role, children })
}

fn leaf(is_menu: bool) -> MenuSearchNode {
    MenuSearchNode {
        is_menu,
        children: Vec::new(),
    }
}

/// A clean child list: an array of elements no longer than one observation
/// reads. Anything else is left to the paged read, which reports it.
fn children(item: &CFType) -> Option<Vec<AXElement>> {
    if item.type_of() != unsafe { core_foundation_sys::array::CFArrayGetTypeID() } {
        return None;
    }
    let array = unsafe { CFArray::<CFType>::wrap_under_get_rule(item.as_CFTypeRef() as _) };
    let limit = agent_desktop_core::ObservationBudget::default().max_children_per_node;
    if array.len() as usize > limit {
        return None;
    }
    array
        .iter()
        .map(|value| super::properties::ax_value::retained_ax_element(&value))
        .collect()
}

fn slot(item: &CFType, is_absent: fn(i32) -> bool) -> Slot {
    use super::properties::node_attribute_decode::{is_null, slot_error};
    if is_null(item) {
        return Slot::Absent;
    }
    match slot_error(item) {
        Some(error) if is_absent(error) => Slot::Absent,
        Some(_) => Slot::Unclear,
        None => Slot::Value(item.clone()),
    }
}

fn is_absent_property_error(error: i32) -> bool {
    super::properties::ax_absence::is_absent_attribute_error(error)
}

fn is_absent_children_error(error: i32) -> bool {
    error == accessibility_sys::kAXErrorAttributeUnsupported
        || error == accessibility_sys::kAXErrorNoValue
}

#[cfg(test)]
#[path = "menu_search_node_tests.rs"]
mod tests;
