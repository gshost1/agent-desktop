use agent_desktop_core::AccessibilityNode;

use super::{FrameNodes, graft_frames};
use crate::NodeOrigin;
use crate::tree::web_node::WebNode;

fn node(frame_id: &str, backend: i64) -> WebNode {
    WebNode {
        node: AccessibilityNode {
            ref_id: None,
            role: "group".into(),
            identity: Default::default(),
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: false,
            children: Vec::new(),
        },
        origin: Some(NodeOrigin {
            target_id: "T".into(),
            frame_id: frame_id.into(),
            loader_id: "L".into(),
            backend_node_id: backend,
        }),
        children: Vec::new(),
        run: None,
    }
}

fn frame(frame_id: &str, parent_id: Option<&str>, owner: Option<i64>) -> FrameNodes {
    FrameNodes {
        frame_id: frame_id.into(),
        parent_id: parent_id.map(str::to_owned),
        owner_backend_node_id: owner,
        nodes: vec![node(frame_id, 100)],
    }
}

#[test]
fn a_frame_whose_parent_was_skipped_is_reported_not_silently_dropped() {
    let frames = vec![
        frame("MAIN", None, None),
        frame("ORPHAN", Some("SKIPPED"), Some(5)),
        frame("GRANDCHILD", Some("ORPHAN"), Some(100)),
    ];

    let (main, dropped) = graft_frames(frames);

    assert_eq!(main.len(), 1);
    assert!(main[0].children.is_empty());
    assert_eq!(dropped, ["ORPHAN", "GRANDCHILD"]);
}

#[test]
fn a_child_frame_lands_under_its_owner_element() {
    let mut main = frame("MAIN", None, None);
    main.nodes[0].children.push(node("MAIN", 7));
    let frames = vec![main, frame("CHILD", Some("MAIN"), Some(7))];

    let (roots, dropped) = graft_frames(frames);

    assert!(dropped.is_empty());
    let owner = &roots[0].children[0];
    assert_eq!(owner.children.len(), 1);
    assert_eq!(owner.children[0].origin.as_ref().unwrap().frame_id, "CHILD");
}
