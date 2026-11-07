use super::*;
use serde_json::json;

const RENDERED_KEYS: [&str; 17] = [
    "role",
    "subrole",
    "role_description",
    "name",
    "ref_id",
    "value",
    "description",
    "placeholder",
    "hint",
    "native_id",
    "dom_classes",
    "states",
    "available_actions",
    "bounds",
    "children_count",
    "subtree_truncated",
    "children",
];

fn sample_tree() -> Value {
    json!({
        "role": "window",
        "name": "Weather",
        "native_id": {"kind": "ax_identifier", "value": "SceneWindow"},
        "children": [
            {"role": "group", "available_actions": ["RightClick", "ScrollTo"], "children": [
                {"role": "group", "available_actions": ["SetFocus"], "children": [
                    {"role": "textfield", "name": "Search", "ref_id": "@s8f3k2p9:e3",
                     "value": "Lon", "available_actions": ["Click", "SetValue", "SetFocus"]}
                ]}
            ]},
            {"role": "group", "name": "Location List", "ref_id": "@s8f3k2p9:e4",
             "available_actions": ["Scroll"], "children": [
                {"role": "button", "name": "London, 46°", "ref_id": "@s8f3k2p9:e16",
                 "available_actions": ["Click", "Scroll"], "states": ["selected"]}
            ]},
            {"role": "group", "children_count": 12, "ref_id": "@s8f3k2p9:e20"}
        ]
    })
}

#[test]
fn renders_one_line_per_node_with_local_refs() {
    let outline = render_outline(&sample_tree());
    assert_eq!(
        outline,
        "window \"Weather\" #SceneWindow\n\
         \x20 textfield \"Search\" [e3] = \"Lon\" (SetValue)\n\
         \x20 group \"Location List\" [e4] (Scroll)\n\
         \x20   button \"London, 46°\" [e16] {selected}\n\
         \x20 group [e20] +12\n"
    );
}

#[test]
fn keeps_every_ref_of_the_tree() {
    let outline = render_outline(&sample_tree());
    for local in ["[e3]", "[e4]", "[e16]", "[e20]"] {
        assert!(outline.contains(local), "{local} missing from {outline}");
    }
    assert!(!outline.contains("s8f3k2p9"));
}

#[test]
fn does_not_collapse_a_group_that_carries_information() {
    let tree = json!({"role": "group", "states": ["focused"], "children": [{"role": "button", "name": "OK", "ref_id": "@s1:e1"}]});
    assert_eq!(
        render_outline(&tree),
        "group {focused}\n  button \"OK\" [e1]\n"
    );
}

#[test]
fn does_not_collapse_a_group_with_several_children() {
    let tree = json!({"role": "group", "children": [
        {"role": "button", "name": "A", "ref_id": "@s1:e1"},
        {"role": "button", "name": "B", "ref_id": "@s1:e2"}
    ]});
    assert_eq!(
        render_outline(&tree),
        "group\n  button \"A\" [e1]\n  button \"B\" [e2]\n"
    );
}

#[test]
fn escapes_names_and_marks_truncation_and_bounds() {
    let tree = json!({"role": "statictext", "name": "say \"hi\"\nnow", "subtree_truncated": true,
                      "bounds": {"x": 10.4, "y": 20.6, "width": 100.0, "height": 30.0}});
    assert_eq!(
        render_outline(&tree),
        "statictext \"say \\\"hi\\\"\\nnow\" @10,21 100x30 +…\n"
    );
}

#[test]
fn replaces_tree_with_outline_and_keeps_other_fields() {
    let mut body = json!({"app": "Weather", "snapshot_id": "s8f3k2p9", "ref_count": 4, "complete": true, "tree": sample_tree()});
    replace_tree_with_outline(&mut body);
    assert!(body.get("tree").is_none());
    assert_eq!(body["snapshot_id"], "s8f3k2p9");
    assert!(
        body["outline"]
            .as_str()
            .is_some_and(|text| text.starts_with("window \"Weather\""))
    );
}

#[test]
fn leaves_a_body_without_tree_unchanged() {
    let mut body = json!({"after_action": {"action": "click"}});
    replace_tree_with_outline(&mut body);
    assert_eq!(body, json!({"after_action": {"action": "click"}}));
}

#[test]
fn a_nested_scroller_keeps_its_scroll_marker_and_its_leaves_drop_it() {
    let tree = json!({"role": "scrollarea", "ref_id": "@s1:e1", "available_actions": ["Scroll"], "children": [
        {"role": "list", "ref_id": "@s1:e2", "available_actions": ["Scroll"], "children": [
            {"role": "button", "name": "Row", "ref_id": "@s1:e3", "available_actions": ["Click", "Scroll"]}
        ]}
    ]});
    assert_eq!(
        render_outline(&tree),
        "scrollarea [e1] (Scroll)\n  list [e2] (Scroll)\n    button \"Row\" [e3]\n"
    );
}

#[test]
fn a_clickable_node_without_a_ref_keeps_click() {
    let tree = json!({"role": "button", "name": "Delete", "available_actions": ["Click"]});
    assert_eq!(render_outline(&tree), "button \"Delete\" (Click)\n");
}

#[test]
fn an_identifier_that_could_read_as_syntax_is_quoted() {
    let plain =
        json!({"role": "button", "native_id": {"kind": "ax_identifier", "value": "_NS:61"}});
    let spaced = json!({"role": "button", "native_id": {"kind": "automation_id", "value": "row[e5] (Scroll)"}});
    assert_eq!(render_outline(&plain), "button #_NS:61\n");
    assert_eq!(render_outline(&spaced), "button #\"row[e5] (Scroll)\"\n");
}

#[test]
fn web_descriptors_are_kept() {
    let tree = json!({"role": "radiobutton", "subrole": "AXTabButton", "role_description": "tab",
                      "dom_classes": ["nav", "active"], "name": "Inbox", "ref_id": "@s1:e1"});
    assert_eq!(
        render_outline(&tree),
        "radiobutton/AXTabButton \"Inbox\" [e1] roledesc=\"tab\" .nav.active\n"
    );
}

#[test]
fn a_wrapper_with_a_blank_name_still_collapses() {
    let tree = json!({"role": "group", "name": "  ", "available_actions": ["SetFocus"], "children": [
        {"role": "button", "name": "OK", "ref_id": "@s1:e1"}
    ]});
    assert_eq!(render_outline(&tree), "button \"OK\" [e1]\n");
}

#[test]
fn every_serialized_node_field_is_rendered_or_named() {
    let node = crate::AccessibilityNode {
        ref_id: Some("@s1:e1".into()),
        role: "button".into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name: Some("n".into()),
            value: Some("v".into()),
            description: Some("d".into()),
            native_id: Some(crate::ElementIdentifier {
                kind: crate::IdentifierKind::AxIdentifier,
                value: "id".into(),
            }),
        },
        presentation: crate::NodePresentation {
            hint: Some("h".into()),
            states: vec!["focused".into()],
            available_actions: vec!["Press".into()],
            bounds: Some(crate::Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            }),
            ref_bounds: None,
            descriptors: crate::NodeDescriptor {
                subrole: Some("s".into()),
                role_description: Some("r".into()),
                placeholder: Some("p".into()),
                dom_classes: vec!["c".into()],
            },
        },
        children_count: Some(2),
        subtree_truncated: true,
        children: Vec::new(),
    };
    let value = serde_json::to_value(&node).expect("node serializes");
    for key in value.as_object().expect("node is an object").keys() {
        assert!(
            RENDERED_KEYS.contains(&key.as_str()),
            "outline does not handle `{key}`"
        );
    }
}
