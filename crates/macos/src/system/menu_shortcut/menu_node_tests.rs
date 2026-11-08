use super::*;

fn slots(values: [Option<CFType>; 7]) -> Vec<Option<CFType>> {
    values.into_iter().collect()
}

#[test]
fn a_menu_item_decodes_its_shortcut_and_states() {
    let node = decode_menu_node(&slots([
        Some(CFString::new("AXMenuItem").as_CFType()),
        Some(CFString::new("New Folder").as_CFType()),
        Some(CFString::new("N").as_CFType()),
        Some(CFNumber::from(1_i32).as_CFType()),
        None,
        Some(CFBoolean::true_value().as_CFType()),
        Some(CFBoolean::false_value().as_CFType()),
    ]));
    assert_eq!(node.role.as_deref(), Some("AXMenuItem"));
    assert_eq!(node.title.as_deref(), Some("New Folder"));
    assert_eq!(node.shortcut.character.as_deref(), Some("N"));
    assert_eq!(node.shortcut.modifiers, Some(1));
    assert_eq!(node.shortcut.virtual_key, None);
    assert_eq!(node.enabled, Some(true));
    assert_eq!(node.selected, Some(false));
}

#[test]
fn numeric_booleans_and_virtual_keys_decode() {
    let node = decode_menu_node(&slots([
        None,
        None,
        None,
        Some(CFNumber::from(0_i32).as_CFType()),
        Some(CFNumber::from(51_i32).as_CFType()),
        Some(CFNumber::from(0_i32).as_CFType()),
        Some(CFNumber::from(1_i32).as_CFType()),
    ]));
    assert_eq!(node.shortcut.virtual_key, Some(51));
    assert_eq!(node.enabled, Some(false));
    assert_eq!(node.selected, Some(true));
}

#[test]
fn malformed_values_read_as_absent_rather_than_as_a_shortcut() {
    let node = decode_menu_node(&slots([
        Some(CFNumber::from(3_i32).as_CFType()),
        None,
        Some(CFNumber::from(78_i32).as_CFType()),
        Some(CFNumber::from(256_i32).as_CFType()),
        Some(CFNumber::from(-1_i32).as_CFType()),
        None,
        None,
    ]));
    assert_eq!(node.role, None);
    assert_eq!(node.shortcut, MenuItemShortcut::default());
}

#[test]
fn the_attribute_list_covers_every_decoded_slot() {
    assert_eq!(
        menu_node_attribute_names().len() as usize,
        MENU_NODE_ATTRIBUTES.len()
    );
    assert_eq!(MENU_NODE_ATTRIBUTES[2], "AXMenuItemCmdChar");
    assert_eq!(MENU_NODE_ATTRIBUTES[3], "AXMenuItemCmdModifiers");
    assert_eq!(MENU_NODE_ATTRIBUTES[4], "AXMenuItemCmdVirtualKey");
}
