use agent_desktop_core::{AdapterError, Deadline};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    number::CFNumber,
    string::CFString,
};

use super::{menu_item_shortcut::MenuItemShortcut, shortcut_target::decode_menu_modifiers};
use crate::tree::AXElement;

pub(crate) const MENU_NODE_ATTRIBUTES: [&str; 7] = [
    "AXRole",
    "AXTitle",
    "AXMenuItemCmdChar",
    "AXMenuItemCmdModifiers",
    "AXMenuItemCmdVirtualKey",
    "AXEnabled",
    "AXSelected",
];

/// One menu-bar element as a single batched attribute read sees it.
#[derive(Debug, Default)]
pub(crate) struct MenuNode {
    pub(crate) role: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) shortcut: MenuItemShortcut,
    pub(crate) enabled: Option<bool>,
    pub(crate) selected: Option<bool>,
}

pub(crate) fn menu_node_attribute_names() -> CFArray<CFString> {
    let names = MENU_NODE_ATTRIBUTES.map(CFString::new);
    CFArray::from_CFTypes(&names)
}

/// Reads every attribute the lookup compares in one IPC round trip. A slot the
/// application could not answer in time makes the whole node unreadable, so a
/// busy menu is reported as an incomplete lookup instead of as no shortcut.
pub(crate) fn read_menu_node(
    element: &AXElement,
    names: &CFArray<CFString>,
    deadline: Deadline,
) -> Result<MenuNode, AdapterError> {
    let (error, values) = crate::tree::properties::ax_ipc::copy_multiple_attribute_values(
        element,
        names.as_concrete_TypeRef(),
        deadline,
    );
    deadline.ensure_remaining()?;
    let values = crate::tree::properties::cf_type::created_cf_array(values);
    if error != accessibility_sys::kAXErrorSuccess {
        return Err(crate::tree::surface_read::map_error(
            "AXMenuItemCmdChar",
            error,
        ));
    }
    let values = values
        .filter(|values| values.len() as usize == MENU_NODE_ATTRIBUTES.len())
        .ok_or_else(|| {
            crate::tree::surface_read::map_error(
                "AXMenuItemCmdChar",
                accessibility_sys::kAXErrorFailure,
            )
        })?;
    let slots: Vec<Option<CFType>> = values
        .iter()
        .map(|item| present_slot(&item))
        .collect::<Result<_, _>>()?;
    Ok(decode_menu_node(&slots))
}

fn present_slot(item: &CFType) -> Result<Option<CFType>, AdapterError> {
    use crate::tree::properties::node_attribute_decode::{is_null, slot_error};
    if is_null(item) {
        return Ok(None);
    }
    match slot_error(item) {
        Some(error) if error == accessibility_sys::kAXErrorCannotComplete => Err(
            crate::tree::surface_read::map_error("AXMenuItemCmdChar", error),
        ),
        Some(_) => Ok(None),
        None => Ok(Some(item.clone())),
    }
}

fn decode_menu_node(slots: &[Option<CFType>]) -> MenuNode {
    let slot = |index: usize| slots.get(index).and_then(Option::as_ref);
    MenuNode {
        role: slot(0).and_then(string),
        title: slot(1).and_then(string),
        shortcut: MenuItemShortcut {
            character: slot(2).and_then(string),
            modifiers: slot(3).and_then(integer).and_then(decode_menu_modifiers),
            virtual_key: slot(4)
                .and_then(integer)
                .and_then(|code| u16::try_from(code).ok()),
        },
        enabled: slot(5).and_then(boolean),
        selected: slot(6).and_then(boolean),
    }
}

fn string(item: &CFType) -> Option<String> {
    item.downcast::<CFString>().map(|value| value.to_string())
}

fn integer(item: &CFType) -> Option<i64> {
    item.downcast::<CFNumber>()?.to_i64()
}

fn boolean(item: &CFType) -> Option<bool> {
    if let Some(value) = item.downcast::<CFBoolean>() {
        return Some(bool::from(value));
    }
    integer(item).map(|value| value != 0)
}

#[cfg(test)]
#[path = "menu_node_tests.rs"]
mod tests;
