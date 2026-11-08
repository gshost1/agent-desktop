use agent_desktop_core::tree::roles::{INTERACTIVE_ROLES, is_canonical_role};

use super::{is_frame_owner_role, is_text_fragment_role, map_cdp_role};

const CHROMIUM_ROLES: &[&str] = &[
    "RootWebArea",
    "button",
    "PopUpButton",
    "combobox",
    "link",
    "textbox",
    "searchbox",
    "checkbox",
    "radio",
    "switch",
    "listbox",
    "option",
    "MenuListOption",
    "MenuListPopup",
    "menu",
    "menubar",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "list",
    "listitem",
    "heading",
    "StaticText",
    "paragraph",
    "image",
    "img",
    "tab",
    "tablist",
    "tabpanel",
    "table",
    "grid",
    "tree",
    "treegrid",
    "treeitem",
    "row",
    "cell",
    "gridcell",
    "columnheader",
    "rowheader",
    "slider",
    "spinbutton",
    "scrollbar",
    "progressbar",
    "meter",
    "separator",
    "toolbar",
    "radiogroup",
    "dialog",
    "alertdialog",
    "alert",
    "status",
    "log",
    "marquee",
    "timer",
    "math",
    "note",
    "tooltip",
    "definition",
    "term",
    "form",
    "main",
    "navigation",
    "banner",
    "contentinfo",
    "complementary",
    "region",
    "search",
    "article",
    "document",
    "DisclosureTriangle",
    "ColorWell",
    "date",
    "InputTime",
    "generic",
    "none",
    "LabelText",
    "Iframe",
    "group",
    "Section",
    "figure",
];

#[test]
fn every_mapped_role_is_in_the_core_vocabulary() {
    for role in CHROMIUM_ROLES {
        let mapped = map_cdp_role(role);
        assert!(is_canonical_role(mapped), "{role} -> {mapped}");
        assert_ne!(mapped, "unknown", "{role}");
    }
}

#[test]
fn interactive_web_controls_map_to_interactive_core_roles() {
    for (cdp, core) in [
        ("button", "button"),
        ("link", "link"),
        ("textbox", "textfield"),
        ("searchbox", "textfield"),
        ("checkbox", "checkbox"),
        ("radio", "radiobutton"),
        ("combobox", "combobox"),
        ("PopUpButton", "combobox"),
        ("option", "option"),
        ("MenuListOption", "option"),
        ("menuitem", "menuitem"),
        ("tab", "tab"),
        ("switch", "switch"),
        ("slider", "slider"),
        ("treeitem", "treeitem"),
        ("gridcell", "cell"),
    ] {
        assert_eq!(map_cdp_role(cdp), core, "{cdp}");
        assert!(INTERACTIVE_ROLES.contains(&core), "{core}");
    }
}

#[test]
fn structure_matches_the_macos_ax_path() {
    assert_eq!(map_cdp_role("RootWebArea"), "webarea");
    assert_eq!(map_cdp_role("listitem"), "group");
    assert_eq!(map_cdp_role("generic"), "group");
    assert_eq!(map_cdp_role("StaticText"), "statictext");
    assert_eq!(map_cdp_role("heading"), "heading");
    assert_eq!(map_cdp_role("list"), "list");
}

#[test]
fn fragment_and_frame_owner_roles_are_recognized() {
    assert!(is_text_fragment_role("InlineTextBox"));
    assert!(is_text_fragment_role("ListMarker"));
    assert!(!is_text_fragment_role("StaticText"));
    assert!(is_frame_owner_role("Iframe"));
    assert!(!is_frame_owner_role("group"));
}
