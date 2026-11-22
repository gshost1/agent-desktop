use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Address, EDITABLE, LABEL, LABEL_FOR, LABELLED_BY, NULL_PATH};
use crate::tree::element::ancestry;
use crate::tree::live::test_support::read_of;
use crate::tree::vocabulary::composite_sibling::{CHILDREN_IN_FLIGHT, EDITABLE_TEXT};
use crate::tree::vocabulary::label_read::{RelationSet, read_label_if_needed};
use crate::tree::vocabulary::role_numbers::{COMBO_BOX, DOCUMENT_WEB, ENTRY, FILLER, PUSH_BUTTON};
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
    refuses: bool,
    lacks_interfaces: bool,
    calls: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    fn get_role(&self) -> u32 {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.role
    }

    fn get_state(&self) -> zbus::fdo::Result<Vec<u32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.refuses {
            true => Err(zbus::fdo::Error::Failed("busy".to_owned())),
            false => Ok(vec![u32::from(self.editable) << EDITABLE, 0]),
        }
    }

    fn get_interfaces(&self) -> zbus::fdo::Result<Vec<&str>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.lacks_interfaces {
            true => Err(zbus::fdo::Error::UnknownMethod("GetInterfaces".to_owned())),
            false => Ok(self.interfaces.clone()),
        }
    }

    fn get_children(&self) -> Vec<Address> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.children.iter().map(|path| address(path)).collect()
    }

    fn get_relation_set(&self) -> RelationSet {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.relations
            .iter()
            .map(|(kind, path)| (*kind, vec![address(path)]))
            .collect()
    }

    #[zbus(property)]
    fn parent(&self) -> Address {
        self.calls.fetch_add(1, Ordering::SeqCst);
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

fn entry(calls: &Arc<AtomicUsize>) -> Accessible {
    Accessible {
        editable: true,
        parent: "/composite",
        calls: calls.clone(),
        ..Accessible::default()
    }
}

fn serve(objects: impl IntoIterator<Item = (&'static str, Accessible)>) -> [zbus::Connection; 2] {
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = Builder::async_io_unix_stream(far)
        .server(zbus::Guid::generate())
        .unwrap()
        .p2p();
    for (path, object) in objects {
        server = server.serve_at(path, object).unwrap();
    }
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        server.build(),
    ));
    [client.unwrap(), server.unwrap()]
}

fn labels_of(
    connection: &zbus::Connection,
    entries: &[&str],
    in_web_document: bool,
) -> Vec<LabelOutcome> {
    let reads = entries.iter().map(|path| {
        let mut read = read_of(ENTRY, "", &[]);
        read.node.element = AtspiElement::new(OWNER, *path, 1);
        if in_web_document {
            let document = AtspiElement::new(OWNER, "/document", 1);
            read.node
                .element
                .set_ancestry(ancestry::child(&document, Some(DOCUMENT_WEB)));
        }
        read.node.traits.states = Field::Present(vec![1 << EDITABLE, 0]);
        read
    });
    let reads: Vec<_> = reads.collect();
    zbus::block_on(futures_util::future::join_all(
        reads
            .iter()
            .map(|read| read_label_if_needed(connection, &read.node)),
    ))
    .into_iter()
    .map(Result::unwrap)
    .collect()
}

fn label_for(
    parent: Accessible,
    siblings: impl IntoIterator<Item = (&'static str, Accessible)>,
    label: Accessible,
) -> LabelOutcome {
    let calls = Arc::default();
    let objects = [
        ("/composite", parent),
        ("/label", label),
        ("/entry", entry(&calls)),
    ];
    let [client, _server] = serve(objects.into_iter().chain(siblings));
    labels_of(&client, &["/entry"], false).remove(0)
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

#[test]
fn a_vanished_sibling_is_absent_but_a_refusing_one_leaves_the_label_unknown() {
    for (refusing, expected) in [
        (None, LabelOutcome::Text("Find:".to_owned())),
        (Some(false), LabelOutcome::Text("Find:".to_owned())),
        (Some(true), LabelOutcome::Failed),
    ] {
        let parent = Accessible {
            children: vec!["/entry", "/vanished", "/toggle"],
            relations: vec![(LABELLED_BY, "/label")],
            ..Accessible::default()
        };
        let toggle = refusing.map(|refuses| {
            (
                "/toggle",
                Accessible {
                    refuses,
                    ..Accessible::default()
                },
            )
        });
        assert_eq!(label_for(parent, toggle, label(Vec::new())), expected);
    }
}

#[test]
fn a_live_sibling_without_an_interface_read_leaves_the_label_unknown() {
    let lacking = || Accessible {
        editable: true,
        lacks_interfaces: true,
        ..Accessible::default()
    };
    let labelled = Accessible {
        children: vec!["/entry", "/lacking"],
        relations: vec![(LABELLED_BY, "/label")],
        ..Accessible::default()
    };
    assert_eq!(
        label_for(labelled, [("/lacking", lacking())], label(Vec::new())),
        LabelOutcome::Failed
    );
    let wrapper = Accessible {
        role: FILLER,
        children: vec!["/entry", "/combo", "/lacking"],
        ..Accessible::default()
    };
    let combo = Accessible {
        role: COMBO_BOX,
        relations: vec![(LABELLED_BY, "/label")],
        ..Accessible::default()
    };
    assert_eq!(
        label_for(
            wrapper,
            [("/combo", combo), ("/lacking", lacking())],
            label(vec![(LABEL_FOR, "/composite")])
        ),
        LabelOutcome::Failed
    );
}

fn composite(children: Vec<&'static str>, calls: &Arc<AtomicUsize>) -> Accessible {
    Accessible {
        children,
        relations: vec![(LABELLED_BY, "/label")],
        calls: calls.clone(),
        ..Accessible::default()
    }
}

#[test]
fn reading_stops_at_the_second_editable_child_and_reads_crowded_parents() {
    let siblings: Vec<&'static str> = (0..2 * CHILDREN_IN_FLIGHT)
        .map(|index| &*format!("/s{index}").leak())
        .collect();
    for (second_editable, expected, sibling_calls) in [
        (true, LabelOutcome::Unlabelled, 2 * CHILDREN_IN_FLIGHT),
        (
            false,
            LabelOutcome::Text("Find:".to_owned()),
            4 * CHILDREN_IN_FLIGHT,
        ),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let children = std::iter::once("/entry").chain(siblings.iter().copied());
        let objects = siblings.iter().enumerate().map(|(index, path)| {
            let sibling = Accessible {
                editable: second_editable && index == 0,
                calls: calls.clone(),
                ..Accessible::default()
            };
            (*path, sibling)
        });
        let [client, _server] = serve(
            [
                ("/composite", composite(children.collect(), &Arc::default())),
                ("/label", label(Vec::new())),
                ("/entry", entry(&Arc::default())),
            ]
            .into_iter()
            .chain(objects),
        );
        assert_eq!(labels_of(&client, &["/entry"], false), [expected]);
        assert_eq!(calls.load(Ordering::SeqCst), sibling_calls);
    }
}

#[test]
fn fields_of_one_parent_share_one_read_of_it() {
    let fields = ["/entry", "/e2", "/e3", "/e4"];
    let calls = Arc::new(AtomicUsize::new(0));
    let entries = fields.map(|path| (path, entry(&Arc::default())));
    let [client, _server] = serve(
        [
            ("/composite", composite(fields.to_vec(), &calls)),
            ("/label", label(Vec::new())),
        ]
        .into_iter()
        .chain(entries),
    );
    assert_eq!(
        labels_of(&client, &fields, false),
        [const { LabelOutcome::Unlabelled }; 4]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn a_field_inside_a_web_document_reads_no_composite() {
    let calls = Arc::new(AtomicUsize::new(0));
    let [client, _server] = serve([
        ("/composite", composite(vec!["/entry"], &calls)),
        ("/label", label(Vec::new())),
        ("/entry", entry(&calls)),
    ]);
    assert_eq!(
        labels_of(&client, &["/entry"], true),
        [LabelOutcome::Unlabelled]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        labels_of(&client, &["/entry"], false),
        [LabelOutcome::Text("Find:".to_owned())]
    );
}
