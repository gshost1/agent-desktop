use super::{ValueSource, number_text, reads_content, truncate_to_boundary, value_source};
use crate::tree::live::test_support::words;
use crate::tree::vocabulary::role_numbers::{
    COMBO_BOX, ENTRY, PANEL, PASSWORD_TEXT, PROGRESS_BAR, PUSH_BUTTON, SLIDER, TEXT,
};
use crate::tree::vocabulary::state_bits::EDITABLE;
use crate::tree::{AtspiElement, Field, RawNode};

fn node(role: u32, interfaces: &[&str], state_bits: &[u32]) -> RawNode {
    let mut node = RawNode::unreadable(AtspiElement::new(":1.1", "/p", 1), "unused");
    node.traits.role = Field::Present(role);
    node.traits.states = Field::Present(words(state_bits));
    node.traits.interfaces = Field::Present(
        interfaces
            .iter()
            .map(|name| format!("org.a11y.atspi.{name}"))
            .collect(),
    );
    node
}

#[test]
fn document_and_paragraph_text_is_searchable_even_when_read_only() {
    for role in [73, 82, 83, 92, 93, 94, 95, 96] {
        for states in [&[][..], &[EDITABLE][..], &[43][..]] {
            assert_eq!(
                value_source(&node(role, &["Text"], states)),
                ValueSource::Text
            );
        }
        assert_eq!(value_source(&node(role, &[], &[])), ValueSource::None);
        let mut failed = node(role, &["Text"], &[]);
        failed.traits.interfaces = Field::Failed("NoReply".into());
        assert_eq!(value_source(&failed), ValueSource::Unknown);
    }
}

#[test]
fn document_content_is_read_only_for_searches_and_target_checks() {
    use crate::tree::properties::read_purpose::ReadPurpose;
    for role in [73, 82, 83, 92, 93, 94, 95, 96] {
        let node = node(role, &["Text"], &[EDITABLE]);
        for purpose in [
            ReadPurpose::Snapshot,
            ReadPurpose::Count,
            ReadPurpose::Identity,
            ReadPurpose::Resolve,
            ReadPurpose::Sampling,
            ReadPurpose::Element,
            ReadPurpose::Value,
        ] {
            assert!(!reads_content(&node, purpose), "{role} {purpose:?}");
        }
        for purpose in [ReadPurpose::TextSearch, ReadPurpose::TypeTarget] {
            assert!(reads_content(&node, purpose));
        }
    }
    assert!(reads_content(
        &node(79, &["Text"], &[EDITABLE]),
        ReadPurpose::Snapshot
    ));
}

#[test]
fn a_slider_and_a_progress_bar_with_the_value_interface_read_a_number() {
    assert_eq!(
        value_source(&node(SLIDER, &["Value"], &[])),
        ValueSource::Numeric
    );
    assert_eq!(
        value_source(&node(PROGRESS_BAR, &["Value"], &[])),
        ValueSource::Numeric
    );
}

#[test]
fn a_slider_that_does_not_name_the_value_interface_is_never_read() {
    assert_eq!(value_source(&node(SLIDER, &[], &[])), ValueSource::None);
}

#[test]
fn an_entry_and_an_editable_text_node_read_text() {
    assert_eq!(
        value_source(&node(ENTRY, &["Text"], &[])),
        ValueSource::Text
    );
    assert_eq!(
        value_source(&node(TEXT, &["Text"], &[EDITABLE])),
        ValueSource::Text
    );
    assert_eq!(
        value_source(&node(COMBO_BOX, &["Text"], &[])),
        ValueSource::Text
    );
}

#[test]
fn read_only_text_is_content_and_carries_no_value() {
    assert_eq!(value_source(&node(TEXT, &["Text"], &[])), ValueSource::None);
}

#[test]
fn a_password_field_has_no_value_source() {
    assert_eq!(
        value_source(&node(PASSWORD_TEXT, &["Text"], &[EDITABLE])),
        ValueSource::None
    );
}

#[test]
fn a_role_without_a_value_is_never_read() {
    assert_eq!(
        value_source(&node(PUSH_BUTTON, &["Value", "Text"], &[])),
        ValueSource::None
    );
    assert_eq!(
        value_source(&node(PANEL, &["Value", "Text"], &[])),
        ValueSource::None
    );
}

#[test]
fn a_node_whose_role_read_failed_has_an_unknown_value_source() {
    let unreadable = RawNode::unreadable(AtspiElement::new(":1.1", "/p", 1), "failed");
    assert_eq!(value_source(&unreadable), ValueSource::Unknown);
}

#[test]
fn unread_value_prerequisites_cannot_verify_a_click_effect() {
    use crate::tree::capabilities::action_read::NodeView;
    use crate::tree::live::test_support::read_of;
    use crate::tree::observe::node_evidence::{NodeEvidence, evidence_for};
    use crate::tree::properties::liveness::Liveness;
    use zbus::connection::Builder;

    let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(near).p2p().build(),
        Builder::async_io_unix_stream(far)
            .server(zbus::Guid::generate())
            .unwrap()
            .p2p()
            .build(),
    ));
    let client = client.unwrap();
    let _server = server.unwrap();
    let view = |read: crate::tree::ObservedRead| {
        NodeView::new(NodeEvidence {
            evidence: evidence_for(&read, &read.node.element),
            read,
            liveness: Liveness::Alive,
        })
    };
    let mut before = read_of(ENTRY, "Entry", &["click"]);
    before.value.value = Field::Present("unchanged".to_owned());
    let previous = view(before.clone()).observation();
    for prerequisite in ["interfaces", "role", "states"] {
        let mut after = before.clone();
        if prerequisite == "role" {
            after.node.traits.role = Field::Failed("unreadable".to_owned());
        } else if prerequisite == "states" {
            after.node.traits.role = Field::Present(TEXT);
            after.node.traits.interfaces = Field::Present(vec!["org.a11y.atspi.Text".to_owned()]);
            after.node.traits.states = Field::Failed("unreadable".to_owned());
        } else {
            after.node.traits.interfaces = Field::Failed("unreadable".to_owned());
        }
        after.value = zbus::block_on(super::read_value(&client, &after.node, 64)).unwrap();
        assert!(after.value.value.is_failed(), "{prerequisite}");
        assert_eq!(
            view(after).observation().changed_since(&previous),
            None,
            "{prerequisite}"
        );
    }
}

#[test]
fn value_connection_loss_is_not_field_uncertainty() {
    let connection = crate::tree::live::test_support::closed_connection();
    for node in [node(ENTRY, &["Text"], &[]), node(SLIDER, &["Value"], &[])] {
        let error = zbus::block_on(super::read_value(&connection, &node, 64))
            .expect_err("a lost bus must end observation");
        assert_eq!(error.code, agent_desktop_core::ErrorCode::Internal);
    }
}

#[test]
fn text_longer_than_the_budget_is_cut_on_a_character_boundary() {
    let (kept, cut) = truncate_to_boundary("aé€😀".repeat(3), 10);
    assert!(cut);
    assert!(kept.len() <= 10);
    assert!(kept.starts_with("aé€"));
}

#[test]
fn text_within_the_budget_is_untouched() {
    let (kept, cut) = truncate_to_boundary("héllo".to_owned(), 64);
    assert_eq!((kept.as_str(), cut), ("héllo", false));
}

#[test]
fn a_budget_that_lands_inside_a_multibyte_character_backs_up_to_its_start() {
    let (kept, cut) = truncate_to_boundary("€€".to_owned(), 4);
    assert_eq!((kept.as_str(), cut), ("€", true));
    let (empty, cut) = truncate_to_boundary("€".to_owned(), 2);
    assert_eq!((empty.as_str(), cut), ("", true));
}

#[test]
fn numbers_print_without_a_trailing_fraction_and_non_finite_numbers_are_refused() {
    assert_eq!(number_text(50.0).as_deref(), Some("50"));
    assert_eq!(number_text(0.25).as_deref(), Some("0.25"));
    assert_eq!(number_text(f64::NAN), None);
    assert_eq!(number_text(f64::INFINITY), None);
}

#[test]
fn text_is_not_read_on_the_stub_during_resolve_count_or_plain_snapshot_walks() {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::tree::properties::read_purpose::ReadPurpose;
    use crate::tree::vocabulary::role_numbers::HEADING;
    use crate::tree::vocabulary::role_numbers::PARAGRAPH;
    use crate::tree::walk::walker_expand::read_observed;
    use zbus::connection::Builder;

    struct TextStub {
        role: u32,
        text: &'static str,
        reads: Arc<AtomicUsize>,
    }

    #[zbus::interface(name = "org.a11y.atspi.Accessible")]
    impl TextStub {
        fn get_role(&self) -> u32 {
            self.role
        }

        fn get_role_name(&self) -> &str {
            "text"
        }

        fn get_state(&self) -> Vec<u32> {
            words(&[EDITABLE])
        }

        fn get_interfaces(&self) -> Vec<&str> {
            vec!["org.a11y.atspi.Text", "org.a11y.atspi.EditableText"]
        }

        fn get_attributes(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        fn get_children(&self) -> Vec<(String, zbus::zvariant::OwnedObjectPath)> {
            Vec::new()
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
        fn child_count(&self) -> i32 {
            0
        }

        #[zbus(property)]
        fn accessible_id(&self) -> &str {
            ""
        }
    }

    struct TextInterfaceStub {
        text: &'static str,
        reads: Arc<AtomicUsize>,
    }

    #[zbus::interface(name = "org.a11y.atspi.Text")]
    impl TextInterfaceStub {
        #[zbus(property)]
        fn character_count(&self) -> i32 {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.text.chars().count() as i32
        }

        fn get_text(&self, _start: i32, _end: i32) -> String {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.text.to_owned()
        }
    }

    for (role, path, text) in [
        (PARAGRAPH, "/paragraph", "Body paragraph text"),
        (HEADING, "/heading", "Chapter Heading"),
        (94, "/document", "Document body text"),
    ] {
        let reads = Arc::new(AtomicUsize::new(0));
        let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
        let (client, server) = zbus::block_on(futures_lite::future::zip(
            Builder::async_io_unix_stream(near).p2p().build(),
            Builder::async_io_unix_stream(far)
                .server(zbus::Guid::generate())
                .unwrap()
                .p2p()
                .serve_at(
                    path,
                    TextStub {
                        role,
                        text,
                        reads: reads.clone(),
                    },
                )
                .unwrap()
                .serve_at(
                    path,
                    TextInterfaceStub {
                        text,
                        reads: reads.clone(),
                    },
                )
                .unwrap()
                .build(),
        ));
        let client = client.unwrap();
        let _server = server.unwrap();
        let element = AtspiElement::new(":1.1", path, 1);
        for purpose in [
            ReadPurpose::Resolve,
            ReadPurpose::Count,
            ReadPurpose::Snapshot,
            ReadPurpose::Element,
            ReadPurpose::Value,
        ] {
            let (observed, _) =
                zbus::block_on(read_observed(&client, &element, 4096, purpose)).unwrap();
            assert_eq!(observed.value.value, Field::Absent, "{role} {purpose:?}");
            assert_eq!(reads.load(Ordering::SeqCst), 0, "{role} {purpose:?}");
        }
        for purpose in [ReadPurpose::TextSearch, ReadPurpose::TypeTarget] {
            reads.store(0, Ordering::SeqCst);
            let (observed, _) =
                zbus::block_on(read_observed(&client, &element, 4096, purpose)).unwrap();
            assert_eq!(
                observed.value.value,
                Field::Present(text.to_owned()),
                "{role} {purpose:?}"
            );
            assert!(reads.load(Ordering::SeqCst) >= 2, "{role} {purpose:?}");
        }
    }
}
