use super::{container_among, is_item};

fn roles(names: &[&str]) -> Vec<Option<String>> {
    names.iter().map(|name| Some((*name).to_string())).collect()
}

#[test]
fn a_table_beside_its_column_header_is_the_container() {
    let finder_list = roles(&[
        "AXOutline",
        "AXScrollBar",
        "AXStaticText",
        "AXImage",
        "AXStaticText",
        "AXStaticText",
    ]);
    assert_eq!(container_among(&finder_list), Some(0));
    let header_group = roles(&["AXGroup", "AXTable", "AXScrollBar"]);
    assert_eq!(container_among(&header_group), Some(1));
}

#[test]
fn a_lone_wrapper_is_descended_and_loose_items_are_not() {
    assert_eq!(
        container_among(&roles(&["AXScrollBar", "AXGroup"])),
        Some(1)
    );
    assert_eq!(
        container_among(&roles(&["AXButton", "AXButton", "AXButton"])),
        None
    );
    assert_eq!(container_among(&roles(&["AXTable", "AXOutline"])), None);
}

#[test]
fn a_table_or_outline_item_is_a_row_and_never_a_column_or_header() {
    assert!(is_item(Some("AXOutline"), Some("AXRow")));
    assert!(!is_item(Some("AXOutline"), Some("AXColumn")));
    assert!(!is_item(Some("AXTable"), Some("AXGroup")));
    assert!(is_item(Some("AXGroup"), Some("AXButton")));
    assert!(!is_item(Some("AXGroup"), Some("AXScrollBar")));
    assert!(is_item(None, Some("AXCell")));
}
