use super::tests::{Parent, connection, wrapper};
use super::{WrapperParent, parent_summary};
use crate::tree::live::test_support::ref_entry;
use crate::tree::observe::projection::project_node;
use crate::tree::properties::node_read::read_node_for;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::resolve::resolve_candidates::read_candidates;
use crate::tree::resolve::resolve_search::evaluate;
use crate::tree::resolve::searched::Searched;
use crate::tree::resolve::{Source, resolve};
use crate::tree::walk::level::Level;
use crate::tree::walk::walker::walk_levels;
use crate::tree::walk::walker_expand::{LevelReader, read_observed, read_observed_with_parent};
use crate::tree::walk::wrapper::classify;
use crate::tree::{AtspiElement, Field, LabelOutcome, ObservedRead, Walk, WalkLimits};
use agent_desktop_core::{AdapterError, Deadline, LocatorEvidence, RefEntry};
use zbus::zvariant::OwnedObjectPath;

pub(super) struct Bus;

#[zbus::interface(name = "org.freedesktop.DBus")]
impl Bus {
    #[zbus(name = "GetConnectionUnixProcessID")]
    fn get_connection_unix_process_id(&self, _name: &str) -> u32 {
        1
    }
}

fn walk(
    client: &zbus::Connection,
    root: &AtspiElement,
    purpose: ReadPurpose,
) -> Walk<ObservedRead> {
    let limits = WalkLimits::default();
    let deadline = Deadline::standard().unwrap();
    let mut reader = LevelReader::new(client, root, &limits, deadline).for_purpose(purpose);
    walk_levels(
        root.clone(),
        &limits,
        deadline,
        |level| reader.expand(level),
        classify,
    )
    .unwrap()
}

fn listing(bus: &str, path: &str) -> (String, OwnedObjectPath) {
    (bus.into(), path.try_into().unwrap())
}

struct SearchSource(Walk<ObservedRead>);

impl Source for SearchSource {
    fn open_window(&mut self, _entry: &RefEntry) -> Result<AtspiElement, AdapterError> {
        Ok(self.0.root.payload.node.element.clone())
    }

    fn level(&mut self, _element: &AtspiElement) -> Result<Level<AtspiElement>, AdapterError> {
        panic!("the search-only ref has no path tier")
    }

    fn evidence(&mut self, _element: &AtspiElement) -> Result<LocatorEvidence, AdapterError> {
        panic!("the search-only ref has no path tier")
    }

    fn search(
        &mut self,
        _frame: &AtspiElement,
        entry: &RefEntry,
    ) -> Result<Searched, AdapterError> {
        Ok(evaluate(&self.0, entry, false))
    }

    fn stopped_app(&mut self, _pid: u32) -> Option<String> {
        None
    }
}

fn capture_then_resolve(parent: Parent, root_bus: &str, expected: LabelOutcome) {
    let (client, server) = connection(Some(wrapper()), false);
    zbus::block_on(async {
        server.object_server().at("/listing", parent).await.unwrap();
        server
            .object_server()
            .at("/org/freedesktop/DBus", Bus)
            .await
            .unwrap();
    });
    let root = AtspiElement::new(root_bus, "/listing", 1);
    let snapshot = walk(&client, &root, ReadPurpose::Snapshot);
    assert!(snapshot.complete);
    let captured = &snapshot.root.children[0].payload;
    assert_eq!(captured.label, expected);
    let projection = project_node(captured, false, &WalkLimits::default());
    let mut entry = ref_entry(projection.role.known().unwrap());
    entry.identity.name = projection
        .name
        .known()
        .filter(|name| !name.is_empty())
        .cloned();
    entry.identity.description = projection
        .description
        .known()
        .filter(|text| !text.is_empty())
        .cloned();
    entry.identity.native_id = projection.identifiers.preferred_identifier().cloned();
    let mut shape = walk(&client, &root, ReadPurpose::Shape);
    read_candidates(&client, &mut shape, &entry, Deadline::standard().unwrap()).unwrap();
    let resolved = &shape.root.children[0].payload;
    assert_eq!(resolved.label, captured.label);
    let fresh = project_node(resolved, false, &WalkLimits::default());
    assert_eq!(fresh.description, projection.description);
    assert_eq!(fresh.name, projection.name);
    let searched = evaluate(&shape, &entry, false);
    assert!(!searched.unread_region);
    assert_eq!(searched.candidates.len(), 1);
    assert_eq!(searched.candidates[0].element.path(), "/child");
    let resolved = resolve(
        &mut SearchSource(shape),
        &entry,
        Deadline::standard().unwrap(),
    )
    .unwrap();
    assert_eq!(resolved.path(), "/child");
}

#[test]
fn resolution_uses_lister_even_when_parent_property_names_another_node() {
    capture_then_resolve(
        Parent {
            description: "Listing tooltip",
            ..wrapper()
        },
        ":1.1",
        LabelOutcome::WrapperDescription("Listing tooltip".into()),
    );
}

#[test]
fn resolution_counts_filtered_children_instead_of_child_count() {
    capture_then_resolve(
        Parent {
            children: 3,
            listing: Some(vec![
                listing(":1.1", "/child"),
                listing("", "/empty"),
                listing(":1.1", "/org/a11y/atspi/null"),
            ]),
            ..wrapper()
        },
        ":1.1",
        LabelOutcome::WrapperDescription("Paint smooth strokes".into()),
    );
}

#[test]
fn cross_bus_lister_never_supplies_wrapper_description() {
    capture_then_resolve(wrapper(), ":1.2", LabelOutcome::Unlabelled);
}

#[test]
fn first_listing_in_a_level_wins_even_if_later_listing_is_a_wrapper() {
    for first_is_wrapper in [true, false] {
        let (client, server) = connection(
            Some(Parent {
                description: if first_is_wrapper {
                    "First tooltip"
                } else {
                    ""
                },
                ..wrapper()
            }),
            false,
        );
        zbus::block_on(server.object_server().at(
            "/second",
            Parent {
                description: "Second tooltip",
                ..wrapper()
            },
        ))
        .unwrap();
        let root = AtspiElement::new(":1.1", "/parent", 1);
        let mut reader = LevelReader::new(
            &client,
            &root,
            &WalkLimits::default(),
            Deadline::standard().unwrap(),
        );
        let parents = reader
            .expand(&[root, AtspiElement::new(":1.1", "/second", 1)])
            .unwrap();
        let children = reader.expand(&parents[0].children).unwrap();
        assert_eq!(
            children[0].payload.label,
            if first_is_wrapper {
                LabelOutcome::WrapperDescription("First tooltip".into())
            } else {
                LabelOutcome::Unlabelled
            }
        );
    }
}

#[test]
fn live_parent_must_list_the_element_and_filters_invalid_entries() {
    for (children, expected) in [
        (vec![listing(":1.1", "/other")], LabelOutcome::Unlabelled),
        (
            vec![
                listing(":1.1", "/child"),
                listing("", "/empty"),
                listing(":1.1", "/org/a11y/atspi/null"),
            ],
            LabelOutcome::WrapperDescription("Paint smooth strokes".into()),
        ),
    ] {
        let (client, _server) = connection(
            Some(Parent {
                listing: Some(children),
                children: 3,
                ..wrapper()
            }),
            false,
        );
        let (read, _) = zbus::block_on(read_observed(
            &client,
            &AtspiElement::new(":1.1", "/child", 1),
            4096,
            ReadPurpose::Identity,
        ))
        .unwrap();
        assert_eq!(read.label, expected);
    }
}

#[test]
fn identity_purposes_without_children_use_live_wrapper_fallback() {
    let (client, _server) = connection(Some(wrapper()), false);
    let element = AtspiElement::new(":1.1", "/parent", 1);
    for purpose in [
        ReadPurpose::Snapshot,
        ReadPurpose::TextSearch,
        ReadPurpose::TypeTarget,
        ReadPurpose::Sampling,
        ReadPurpose::Count,
        ReadPurpose::Identity,
        ReadPurpose::Resolve,
        ReadPurpose::Element,
    ] {
        assert!(purpose.identity());
        let (node, _) = zbus::block_on(read_node_for(&client, &element, purpose)).unwrap();
        assert!(!node.properties.name.is_failed(), "{purpose:?}");
        assert!(!node.properties.description.is_failed(), "{purpose:?}");
        let summary = parent_summary(&node, purpose);
        if purpose.children() {
            assert_eq!(
                summary,
                Some(Field::Present("Paint smooth strokes".into())),
                "{purpose:?}"
            );
        } else {
            assert!(node.children.is_failed(), "{purpose:?}");
            assert_eq!(summary, None, "{purpose:?}");
        }
        let child = AtspiElement::new(":1.1", "/child", 1);
        let (read, _) = zbus::block_on(read_observed_with_parent(
            &client,
            &child,
            4096,
            purpose,
            summary.map(WrapperParent::Known).as_ref(),
        ))
        .unwrap();
        assert_eq!(
            read.label,
            LabelOutcome::WrapperDescription("Paint smooth strokes".into()),
            "{purpose:?}"
        );
    }
}
