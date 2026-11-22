use super::{needs_description, read_wrapper_description, select_description};
use crate::tree::live::test_support::read_of;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::vocabulary::label_read::read_label_if_needed;
use crate::tree::vocabulary::role_numbers::{PANEL, PUSH_BUTTON, TOGGLE_BUTTON};
use crate::tree::walk::walker_expand::{LevelReader, read_observed};
use crate::tree::{AtspiElement, Field, LabelOutcome, resolve_vocabulary};
use agent_desktop_core::Deadline;
use std::sync::atomic::{AtomicUsize, Ordering};
use zbus::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

struct Child {
    labelled: bool,
    relation_reads: AtomicUsize,
    parent_reads: AtomicUsize,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Child {
    fn get_role(&self) -> u32 {
        TOGGLE_BUTTON
    }
    fn get_role_name(&self) -> &str {
        "toggle button"
    }
    fn get_state(&self) -> Vec<u32> {
        vec![0, 0]
    }
    fn get_interfaces(&self) -> Vec<&str> {
        vec!["org.a11y.atspi.Accessible"]
    }
    fn get_attributes(&self) -> std::collections::BTreeMap<String, String> {
        Default::default()
    }
    fn get_children(&self) -> Vec<(String, OwnedObjectPath)> {
        vec![]
    }
    fn get_relation_set(&self) -> Vec<(u32, Vec<(String, OwnedObjectPath)>)> {
        self.relation_reads.fetch_add(1, Ordering::Relaxed);
        if self.labelled {
            vec![(2, vec![(":1.1".into(), "/label".try_into().unwrap())])]
        } else {
            vec![]
        }
    }
    #[zbus(property)]
    fn name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn description(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn accessible_id(&self) -> &str {
        "stable-child"
    }
    #[zbus(property)]
    fn child_count(&self) -> i32 {
        0
    }
    #[zbus(property)]
    fn parent(&self) -> (String, OwnedObjectPath) {
        self.parent_reads.fetch_add(1, Ordering::Relaxed);
        (":1.1".into(), "/parent".try_into().unwrap())
    }
}

pub(super) struct Parent {
    pub(super) role: u32,
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) children: i32,
    pub(super) role_reads: AtomicUsize,
    pub(super) listing: Option<Vec<(String, OwnedObjectPath)>>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Parent {
    fn get_role(&self) -> u32 {
        self.role_reads.fetch_add(1, Ordering::Relaxed);
        self.role
    }
    fn get_children(&self) -> Vec<(String, OwnedObjectPath)> {
        if let Some(listing) = &self.listing {
            return listing.clone();
        }
        (0..self.children)
            .map(|index| {
                (
                    ":1.1".into(),
                    if index == 0 {
                        "/child".try_into().unwrap()
                    } else {
                        "/other".try_into().unwrap()
                    },
                )
            })
            .collect()
    }
    #[zbus(property)]
    fn name(&self) -> &str {
        self.name
    }
    #[zbus(property)]
    fn description(&self) -> &str {
        self.description
    }
    #[zbus(property)]
    fn child_count(&self) -> i32 {
        self.children
    }
}

pub(super) fn wrapper() -> Parent {
    Parent {
        role: PANEL,
        name: "",
        description: "Paint smooth strokes",
        children: 1,
        role_reads: AtomicUsize::new(0),
        listing: None,
    }
}

pub(super) fn connection(
    parent: Option<Parent>,
    labelled: bool,
) -> (zbus::Connection, zbus::Connection) {
    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let server = Builder::async_io_unix_stream(far)
        .server(zbus::Guid::generate())
        .unwrap()
        .p2p()
        .serve_at(
            "/child",
            Child {
                labelled,
                relation_reads: AtomicUsize::new(0),
                parent_reads: AtomicUsize::new(0),
            },
        )
        .unwrap()
        .serve_at(
            "/label",
            Parent {
                name: "Explicit label",
                ..wrapper()
            },
        )
        .unwrap();
    let server = match parent {
        Some(parent) => server.serve_at("/parent", parent).unwrap(),
        None => server,
    };
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        server.build(),
    ));
    (client.unwrap(), server.unwrap())
}

#[test]
fn sole_child_wrapper_description_is_shared_by_capture_and_live_identity_reads() {
    let (client, _server) = connection(Some(wrapper()), false);
    for purpose in [
        ReadPurpose::Snapshot,
        ReadPurpose::Identity,
        ReadPurpose::Resolve,
        ReadPurpose::Count,
    ] {
        let (read, _) = zbus::block_on(read_observed(
            &client,
            &AtspiElement::new(":1.1", "/child", 1),
            4096,
            purpose,
        ))
        .unwrap();
        assert_eq!(
            read.label,
            LabelOutcome::WrapperDescription("Paint smooth strokes".into()),
            "{purpose:?}"
        );
        let vocabulary = resolve_vocabulary(&read.node, &read.label, false);
        assert_eq!(
            vocabulary.name,
            agent_desktop_core::LocatorField::Known("Paint smooth strokes".into())
        );
        assert!(read.node.properties.name.is_blank());
        assert!(read.node.properties.description.is_blank());
    }
}

#[test]
fn wrapper_description_requires_one_unnamed_noninteractive_container() {
    for parent in [
        Parent {
            children: 2,
            ..wrapper()
        },
        Parent {
            name: "Toolbar",
            ..wrapper()
        },
        Parent {
            role: PUSH_BUTTON,
            ..wrapper()
        },
        Parent {
            description: "  ",
            ..wrapper()
        },
    ] {
        let (client, _server) = connection(Some(parent), false);
        let mut node = read_of(TOGGLE_BUTTON, "", &["click"]).node;
        node.element = AtspiElement::new(":1.1", "/child", 1);
        assert_eq!(
            zbus::block_on(read_label_if_needed(&client, &node)).unwrap(),
            LabelOutcome::Unlabelled
        );
        assert!(node.properties.description.is_blank());
    }
    assert_eq!(
        select_description(
            &Field::Present(String::new()),
            Field::Present("Tooltip".into()),
        ),
        Field::Present("Tooltip".into())
    );
}

#[test]
fn own_identity_and_explicit_labels_suppress_wrapper_reads() {
    let connection = crate::tree::live::test_support::closed_connection();
    for (name, description) in [("Own name", ""), ("", "Own description")] {
        let mut node = read_of(TOGGLE_BUTTON, name, &["click"]).node;
        node.properties.description = Field::Present(description.into());
        assert_eq!(needs_description(&node), description.is_empty());
        if name.is_empty() {
            assert_eq!(
                zbus::block_on(read_wrapper_description(&connection, &node, None)).unwrap(),
                Field::Absent
            );
        } else {
            assert_eq!(
                zbus::block_on(read_label_if_needed(&connection, &node)).unwrap(),
                LabelOutcome::Unlabelled
            );
        }
        assert_eq!(
            node.properties.description.present().map(String::as_str),
            Some(description)
        );
    }
    let (client, _server) = self::connection(None, true);
    let mut node = read_of(TOGGLE_BUTTON, "", &["click"]).node;
    node.element = AtspiElement::new(":1.1", "/child", 1);
    assert_eq!(
        zbus::block_on(read_label_if_needed(&client, &node)).unwrap(),
        LabelOutcome::Text("Explicit label".into())
    );
    assert!(node.properties.description.is_blank());
}

#[test]
fn optional_wrapper_failure_keeps_own_description_and_bus_loss_propagates() {
    let (client, _server) = connection(None, false);
    let mut node = read_of(TOGGLE_BUTTON, "", &["click"]).node;
    node.element = AtspiElement::new(":1.1", "/child", 1);
    let label = zbus::block_on(read_label_if_needed(&client, &node)).unwrap();
    assert_eq!(label, LabelOutcome::WrapperUnread);
    assert!(node.properties.description.is_blank());
    assert!(!node.properties.description.is_failed());
    let vocabulary = resolve_vocabulary(&node, &label, false);
    assert_eq!(vocabulary.name, agent_desktop_core::LocatorField::Unknown);
    node.properties.description = Field::Present(String::new());
    let lost = crate::tree::live::test_support::closed_connection();
    let error = zbus::block_on(read_wrapper_description(&lost, &node, None)).unwrap_err();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Internal);
    assert_eq!(
        select_description(
            &Field::Failed("unread name".into()),
            Field::Present("Tooltip".into()),
        ),
        Field::Failed("Wrapper text unreadable".into())
    );
    assert_eq!(
        select_description(&Field::Absent, Field::Present("Tooltip".into()),),
        Field::Present("Tooltip".into())
    );
}

#[test]
fn wrapper_fallback_reads_labelled_by_only_once() {
    let (client, server) = connection(Some(wrapper()), false);
    let (read, _) = zbus::block_on(read_observed(
        &client,
        &AtspiElement::new(":1.1", "/child", 1),
        4096,
        ReadPurpose::Snapshot,
    ))
    .unwrap();
    assert!(matches!(read.label, LabelOutcome::WrapperDescription(_)));
    let reads = zbus::block_on(async {
        let interface = server
            .object_server()
            .interface::<_, Child>("/child")
            .await
            .unwrap();
        interface.get().await.relation_reads.load(Ordering::Relaxed)
    });
    assert_eq!(reads, 1);
}

#[test]
fn walked_child_reuses_parent_without_parent_calls() {
    let (client, server) = connection(Some(wrapper()), false);
    let root = AtspiElement::new(":1.1", "/parent", 1);
    let mut reader = LevelReader::new(
        &client,
        &root,
        &Default::default(),
        Deadline::standard().unwrap(),
    );
    let parents = reader.expand(&[root]).unwrap();
    let children = reader.expand(&parents[0].children).unwrap();
    assert_eq!(
        children[0].payload.label,
        LabelOutcome::WrapperDescription("Paint smooth strokes".into())
    );
    zbus::block_on(async {
        let child = server
            .object_server()
            .interface::<_, Child>("/child")
            .await
            .unwrap();
        assert_eq!(child.get().await.parent_reads.load(Ordering::Relaxed), 0);
        let parent = server
            .object_server()
            .interface::<_, Parent>("/parent")
            .await
            .unwrap();
        assert_eq!(parent.get().await.role_reads.load(Ordering::Relaxed), 1);
    });
}
