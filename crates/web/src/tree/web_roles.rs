/// Maps a Chromium accessibility role, as `Accessibility.getFullAXTree`
/// reports it, to the core role vocabulary. Matching is case-insensitive.
/// The mapping follows what Chromium exposes through the macOS AX API for
/// the same element, so a page reads the same through either path: list
/// items and generic containers are groups, `<select>` is a combobox, and a
/// role with no closer core equivalent is a group that wrapper collapsing
/// may remove.
pub(crate) fn map_cdp_role(role: &str) -> &'static str {
    match role.to_ascii_lowercase().as_str() {
        "rootwebarea" | "webarea" => "webarea",
        "button" | "togglebutton" => "button",
        "popupbutton"
        | "combobox"
        | "comboboxgrouping"
        | "comboboxmenubutton"
        | "comboboxselect"
        | "textfieldwithcombobox" => "combobox",
        "link" => "link",
        "textbox" | "searchbox" | "textfield" => "textfield",
        "checkbox" => "checkbox",
        "radio" | "radiobutton" => "radiobutton",
        "switch" => "switch",
        "listbox" => "listbox",
        "option" | "listboxoption" | "menulistoption" => "option",
        "menulistpopup" | "menu" | "menubar" => "menu",
        "menuitem" | "menuitemcheckbox" | "menuitemradio" => "menuitem",
        "list" | "descriptionlist" | "directory" => "list",
        "heading" => "heading",
        "statictext" | "text" => "statictext",
        "paragraph" => "paragraph",
        "image" | "img" | "graphics-symbol" | "svgroot" => "image",
        "tab" => "tab",
        "tablist" => "tablist",
        "tabpanel" => "tabpanel",
        "table" => "table",
        "grid" => "grid",
        "tree" | "treegrid" => "outline",
        "treeitem" => "treeitem",
        "row" => "row",
        "cell" | "gridcell" | "columnheader" | "rowheader" => "cell",
        "column" => "column",
        "slider" => "slider",
        "spinbutton" => "incrementor",
        "scrollbar" => "scrollbar",
        "progressindicator" | "progressbar" => "progressbar",
        "meter" => "levelindicator",
        "separator" | "splitter" => "separator",
        "toolbar" => "toolbar",
        "radiogroup" => "radiogroup",
        "dialog" => "dialog",
        "alertdialog" => "alertdialog",
        "alert" => "alert",
        "status" => "status",
        "log" => "log",
        "marquee" => "marquee",
        "timer" => "timer",
        "math" | "mathmlmath" => "math",
        "note" => "note",
        "tooltip" => "tooltip",
        "definition" => "definition",
        "term" | "descriptionlistterm" => "term",
        "form" => "form",
        "main" => "main",
        "navigation" => "navigation",
        "banner" | "header" => "banner",
        "contentinfo" | "footer" => "contentinfo",
        "complementary" => "complementary",
        "region" => "region",
        "search" => "search",
        "article" => "article",
        "document" => "document",
        "disclosuretriangle" => "disclosure",
        "colorwell" => "colorwell",
        "date" | "datetime" => "datefield",
        "inputtime" => "timefield",
        _ => "group",
    }
}

/// Roles whose subtree is rendering detail, never content an agent reads:
/// inline text boxes under static text, list bullets.
pub(crate) fn is_text_fragment_role(role: &str) -> bool {
    matches!(
        role.to_ascii_lowercase().as_str(),
        "inlinetextbox" | "listmarker"
    )
}

/// The role of a `<br>`, which emits no node but separates the text on
/// either side of it.
pub(crate) fn is_line_break_role(role: &str) -> bool {
    role.eq_ignore_ascii_case("linebreak")
}

/// Roles that own a child frame. They are never collapsed, because a child
/// frame's tree is grafted beneath them.
pub(crate) fn is_frame_owner_role(role: &str) -> bool {
    matches!(
        role.to_ascii_lowercase().as_str(),
        "iframe" | "iframepresentational"
    )
}

#[cfg(test)]
#[path = "web_roles_tests.rs"]
mod tests;
