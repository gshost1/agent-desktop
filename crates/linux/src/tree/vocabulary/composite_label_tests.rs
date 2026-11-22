use super::{
    Address, COMBO_BOX, EDITABLE, EDITABLE_TEXT, LABEL, LABEL_FOR, LABELLED_BY, NULL_PATH,
};
use crate::tree::live::test_support::read_of;
use crate::tree::vocabulary::label_read::{RelationSet, read_label_if_needed};
use crate::tree::vocabulary::role_numbers::{ENTRY, FILLER, PUSH_BUTTON};
use crate::tree::{AtspiElement, Field, LabelOutcome};
use zbus::connection::Builder;

const OWNER: &str = "org.agentdesktop.CompositeLabelFixture";

#[derive(Default)]
struct Accessible {
    role: u32,
    editable: bool,
    interfaces: Vec<&'static str>,
    name: &'static str,
    parent: &'static str,
    children: Vec<&'static str>,
    relations: Vec<(u32, &'static str)>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    fn get_role(&self) -> u32 {
        self.role
    }

    fn get_state(&self) -> Vec<u32> {
        vec![u32::from(self.editable) << EDITABLE, 0]
    }

    fn get_interfaces(&self) -> Vec<&str> {
        self.interfaces.clone()
    }

    fn get_children(&self) -> Vec<Address> {
        self.children.iter().map(|path| address(path)).collect()
    }

    fn get_relation_set(&self) -> RelationSet {
        self.relations
            .iter()
            .map(|(kind, path)| (*kind, vec![address(path)]))
            .collect()
    }

    #[zbus(property)]
    fn parent(&self) -> Address {
        address(self.parent)
    }

    #[zbus(property)]
    fn name(&self) -> &str {
        self.name
    }
}

fn address(path: &str) -> Address {
    (OWNER.to_owned(), path.try_into().unwrap())
}

fn label_for(
    parent: Accessible,
    siblings: impl IntoIterator<Item = (&'static str, Accessible)>,
    label: Accessible,
) -> LabelOutcome {
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = Builder::async_io_unix_stream(far)
        .server(zbus::Guid::generate())
        .unwrap()
        .p2p()
        .serve_at("/composite", parent)
        .unwrap()
        .serve_at("/label", label)
        .unwrap()
        .serve_at(
            "/entry",
            Accessible {
                parent: "/composite",
                ..Accessible::default()
            },
        )
        .unwrap();
    for (path, sibling) in siblings {
        server = server.serve_at(path, sibling).unwrap();
    }
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        server.build(),
    ));
    let _server = server.unwrap();
    let mut read = read_of(ENTRY, "", &[]);
    read.node.element = AtspiElement::new(OWNER, "/entry", 1);
    read.node.traits.states = Field::Present(vec![1 << EDITABLE, 0]);
    zbus::block_on(read_label_if_needed(&client.unwrap(), &read.node)).unwrap()
}

fn label(relations: Vec<(u32, &'static str)>) -> Accessible {
    Accessible {
        role: LABEL,
        name: "Find:",
        relations,
        ..Accessible::default()
    }
}

#[test]
fn an_explicitly_labelled_combo_names_its_only_editable_child() {
    let parent = Accessible {
        role: COMBO_BOX,
        children: vec!["/entry", "/toggle"],
        relations: vec![(LABELLED_BY, "/label")],
        ..Accessible::default()
    };
    let toggle = Accessible {
        role: PUSH_BUTTON,
        ..Accessible::default()
    };
    assert_eq!(
        label_for(parent, [("/toggle", toggle)], label(Vec::new())),
        LabelOutcome::Text("Find:".to_owned())
    );
}

#[test]
fn a_sibling_combos_label_must_explicitly_label_the_shared_wrapper() {
    for (label_for_path, expected) in [
        ("/composite", LabelOutcome::Text("Find:".to_owned())),
        ("/combo", LabelOutcome::Unlabelled),
    ] {
        let parent = Accessible {
            role: FILLER,
            children: vec!["/entry", "/combo"],
            ..Accessible::default()
        };
        let combo = Accessible {
            role: COMBO_BOX,
            relations: vec![(LABELLED_BY, "/label")],
            ..Accessible::default()
        };
        assert_eq!(
            label_for(
                parent,
                [("/combo", combo)],
                label(vec![(LABEL_FOR, label_for_path)])
            ),
            expected
        );
    }
}

#[test]
fn a_combo_cannot_pass_on_a_label_that_names_another_control() {
    let parent = Accessible {
        role: COMBO_BOX,
        children: vec!["/entry"],
        relations: vec![(LABELLED_BY, "/label")],
        ..Accessible::default()
    };
    assert_eq!(
        label_for(parent, [], label(vec![(LABEL_FOR, "/another")])),
        LabelOutcome::Unlabelled
    );
}

#[test]
fn a_second_editable_child_refuses_the_label_even_without_a_showing_state() {
    for (editable, interfaces) in [(true, Vec::new()), (false, vec![EDITABLE_TEXT])] {
        let parent = Accessible {
            children: vec!["/entry", "/second"],
            relations: vec![(LABELLED_BY, "/label")],
            ..Accessible::default()
        };
        let second = Accessible {
            role: ENTRY,
            editable,
            interfaces,
            ..Accessible::default()
        };
        assert_eq!(
            label_for(parent, [("/second", second)], label(Vec::new())),
            LabelOutcome::Unlabelled
        );
    }
}

#[test]
fn unread_or_unconfirmed_siblings_cannot_prove_ownership() {
    for (children, expected) in [
        (vec!["/entry", "/missing"], LabelOutcome::Failed),
        (vec!["/entry", NULL_PATH], LabelOutcome::Unlabelled),
        (vec!["/entry", "/entry"], LabelOutcome::Unlabelled),
        (vec![], LabelOutcome::Unlabelled),
    ] {
        let parent = Accessible {
            children,
            relations: vec![(LABELLED_BY, "/label")],
            ..Accessible::default()
        };
        assert_eq!(label_for(parent, [], label(Vec::new())), expected);
    }
}
