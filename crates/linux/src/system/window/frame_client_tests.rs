use super::*;
use crate::system::x11::FrameExtents;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::vocabulary::role_numbers::FRAME;
use crate::tree::vocabulary::state_bits::SHOWING;
use crate::tree::{COMPONENT_INTERFACE, Extents, Field};

fn gtk2() -> ToolkitId {
    ToolkitId {
        name: Some("gail".into()),
        version: Some("2.24.33".into()),
    }
}

fn node(path: &str, title: &str, rect: (i32, i32, i32, i32)) -> RawNode {
    let mut node = read_of(FRAME, title, &[]).node;
    node.element = AtspiElement::new(":1.133", path, 20341);
    node.traits.interfaces = Field::Present(vec![COMPONENT_INTERFACE.into()]);
    node.traits.states = Field::Present(words(&[SHOWING]));
    node.extents = Field::Present(Extents {
        x: rect.0,
        y: rect.1,
        width: rect.2,
        height: rect.3,
    });
    node
}

fn gimp_state() -> (Vec<RawNode>, Vec<X11Client>) {
    let title = "*[Untitled]-1.0 (RGB color 8-bit gamma integer, GIMP built-in sRGB, 1 layer) 320x240 – GIMP";
    let nodes = vec![
        node("/main", title, (200, 100, 1026, 793)),
        node("/export", "Export Image", (166, 129, 950, 668)),
        node("/question", "Question", (380, 384, 524, 194)),
    ];
    let clients = [
        (4194307, title, (201, 120, 1024, 768), 5),
        (4197463, "Export Image", (167, 149, 948, 643), 5),
        (4198305, "", (381, 404, 522, 173), 1),
    ]
    .into_iter()
    .map(|(id, title, bounds, bottom)| X11Client {
        id,
        pid: Some(20341),
        title: title.into(),
        bounds: Some(Extents {
            x: bounds.0,
            y: bounds.1,
            width: bounds.2,
            height: bounds.3,
        }),
        extents: Some(FrameExtents {
            left: 1,
            right: 1,
            top: 20,
            bottom,
        }),
        hidden: false,
        shadow: (0, 0, 0, 0),
    })
    .collect();
    (nodes, clients)
}

#[test]
fn a_gimp_question_uses_its_unique_proved_size_despite_its_native_title() {
    let (nodes, clients) = gimp_state();
    assert_eq!(
        match_frame(&nodes[2].element, &nodes, &gtk2(), &clients),
        ClientMatch::One(4198305)
    );
}

#[test]
fn geometry_cannot_reclaim_a_client_a_named_sibling_owns() {
    let (nodes, mut clients) = gimp_state();
    clients[1].title = "Different export title".into();
    clients[2].title = "Export Image".into();
    assert_eq!(
        match_frame(&nodes[2].element, &nodes, &gtk2(), &clients),
        ClientMatch::NoClient
    );
}

#[test]
fn different_unique_sizes_cannot_share_one_clients_inner_and_outer_geometry() {
    let (mut nodes, clients) = gimp_state();
    nodes.push(node("/other", "Other", (0, 0, 522, 173)));
    for index in [2, 3] {
        assert!(permits_size_match(
            &nodes[index],
            &nodes,
            &gtk2(),
            nodes[index].extents.present().copied()
        ));
        assert_eq!(
            match_frame(&nodes[index].element, &nodes, &gtk2(), &clients),
            ClientMatch::NoClient
        );
    }
}

#[test]
fn geometry_without_a_size_identity_proof_is_refused() {
    let (original, clients) = gimp_state();
    for failure in 0..4 {
        let mut nodes = original.clone();
        match failure {
            0 => nodes.push(node("/other", "Other", (0, 0, 524, 194))),
            1 => nodes[0].extents = Field::Failed("unanswered size".into()),
            2 => nodes[0].traits.interfaces = Field::Failed("unanswered interfaces".into()),
            _ => nodes[2].traits.states = Field::Present(words(&[])),
        }
        assert_eq!(
            match_frame(&nodes[2].element, &nodes, &gtk2(), &clients),
            ClientMatch::NoClient,
            "failure {failure}"
        );
    }
}

#[test]
fn a_missing_frame_or_client_cannot_supply_an_origin() {
    let (nodes, clients) = gimp_state();
    let target = &nodes[2].element;
    assert_eq!(
        match_frame(target, &[], &gtk2(), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_frame(target, &nodes[..2], &gtk2(), &clients),
        ClientMatch::NoClient
    );
    assert_eq!(
        match_frame(target, &nodes, &gtk2(), &[]),
        ClientMatch::NoClient
    );
}
