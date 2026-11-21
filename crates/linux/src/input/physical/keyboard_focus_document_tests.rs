use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{DeliverySemantics, ErrorCode};
use zbus::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

use super::{
    EDITABLE_TEXT, InputTarget, TEXT_INTERFACE, focused, focused_document_descendant,
    internal_editor_focused, needs_document_routing,
};
use crate::tree::AtspiElement;
use crate::tree::element::ancestry;
use crate::tree::vocabulary::role_numbers::{DOCUMENT_FRAME, HEADING, LABEL, PARAGRAPH};
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED, READ_ONLY};

const OWNER: &str = "org.agentdesktop.DocumentFocusFixture";

struct DocNode {
    role: u32,
    states: Option<u32>,
    interfaces: Option<Vec<&'static str>>,
    children: Vec<(&'static str, &'static str)>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl DocNode {
    fn get_role(&self) -> u32 {
        self.role
    }

    fn get_state(&self) -> Result<Vec<u32>, zbus::fdo::Error> {
        self.states
            .map(|bits| vec![bits, 0])
            .ok_or_else(|| zbus::fdo::Error::Failed("state read failed".into()))
    }

    fn get_interfaces(&self) -> Result<Vec<&str>, zbus::fdo::Error> {
        self.interfaces
            .clone()
            .ok_or_else(|| zbus::fdo::Error::Failed("interface read failed".into()))
    }

    fn get_attributes(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn get_children(&self) -> Vec<(String, OwnedObjectPath)> {
        self.children
            .iter()
            .map(|(bus, path)| ((*bus).to_owned(), (*path).try_into().expect("child path")))
            .collect()
    }
}

struct CountingText {
    reads: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.a11y.atspi.Text")]
impl CountingText {
    #[zbus(property)]
    fn character_count(&self) -> i32 {
        self.reads.fetch_add(1, Ordering::SeqCst);
        5
    }

    fn get_text(&self, _start: i32, _end: i32) -> String {
        self.reads.fetch_add(1, Ordering::SeqCst);
        "hello".into()
    }
}

fn bits(list: &[u32]) -> u32 {
    list.iter().fold(0u32, |mask, bit| mask | (1u32 << bit))
}

fn node(role: u32, states: &[u32], children: Vec<(&'static str, &'static str)>) -> DocNode {
    DocNode {
        role,
        states: Some(bits(states)),
        interfaces: Some(vec![TEXT_INTERFACE, EDITABLE_TEXT]),
        children,
    }
}

fn serve(nodes: Vec<(&'static str, DocNode)>) -> (zbus::Connection, zbus::Connection, Arc<AtomicUsize>) {
    let reads = Arc::new(AtomicUsize::new(0));
    let (near, far) = std::os::unix::net::UnixStream::pair().expect("socket pair");
    let (client, server) = zbus::block_on(async {
        let mut builder = Builder::async_io_unix_stream(far)
            .server(zbus::Guid::generate())
            .expect("guid")
            .p2p();
        for (path, obj) in nodes {
            builder = builder
                .serve_at(path, obj)
                .expect("accessible")
                .serve_at(
                    path,
                    CountingText {
                        reads: reads.clone(),
                    },
                )
                .expect("text");
        }
        futures_lite::future::zip(
            Builder::async_io_unix_stream(near).p2p().build(),
            builder.build(),
        )
        .await
    });
    (client.expect("client"), server.expect("server"), reads)
}

fn doc_root() -> AtspiElement {
    let frame = AtspiElement::new(OWNER, "/frame", 7);
    let mut doc = AtspiElement::new(OWNER, "/document", 7);
    doc.set_ancestry(ancestry::of_root(&frame));
    doc
}

#[test]
fn caret_in_heading_and_caret_in_paragraph_both_route_with_role_path_and_ancestry() {
    for (focused_path, expected_role) in [("/heading", "heading"), ("/paragraph", "paragraph")] {
        let (client, _server, reads) = serve(vec![
            (
                "/document",
                DocNode {
                    role: 94,
                    states: Some(bits(&[EDITABLE])),
                    interfaces: Some(vec![]),
                    children: vec![(OWNER, "/heading"), (OWNER, "/paragraph")],
                },
            ),
            (
                "/heading",
                node(
                    HEADING,
                    if focused_path == "/heading" {
                        &[EDITABLE, FOCUSED]
                    } else {
                        &[EDITABLE]
                    },
                    vec![],
                ),
            ),
            (
                "/paragraph",
                node(
                    PARAGRAPH,
                    if focused_path == "/paragraph" {
                        &[EDITABLE, FOCUSED]
                    } else {
                        &[EDITABLE]
                    },
                    vec![],
                ),
            ),
        ]);
        let root = doc_root();
        assert!(zbus::block_on(focused(&client, &root)).unwrap());
        let editor = zbus::block_on(internal_editor_focused(&client, &root))
            .unwrap()
            .expect("uniquely focused descendant");
        assert_eq!(editor.path(), focused_path);
        assert_eq!(editor.resolved_role(), Some(expected_role));
        assert_eq!(editor.frame().map(AtspiElement::path), Some("/frame"));
        let input = InputTarget::new(editor);
        assert_eq!(
            input.details,
            serde_json::json!({"role": expected_role, "path": focused_path})
        );
        assert_eq!(
            reads.load(Ordering::SeqCst),
            0,
            "focus routing must never read Text content"
        );
    }
}

#[test]
fn several_focused_candidates_and_none_fail_closed() {
    let cases: Vec<(&str, Vec<(&'static str, DocNode)>)> = vec![
        (
            "empty document",
            vec![(
                "/document",
                DocNode {
                    role: DOCUMENT_FRAME,
                    states: Some(bits(&[EDITABLE])),
                    interfaces: Some(vec![]),
                    children: vec![],
                },
            )],
        ),
        (
            "none focused",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/heading"), (OWNER, "/p1")],
                    },
                ),
                ("/heading", node(HEADING, &[EDITABLE], vec![])),
                ("/p1", node(PARAGRAPH, &[EDITABLE], vec![])),
            ],
        ),
        (
            "two focused descendants (heading + paragraph)",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/heading"), (OWNER, "/p1")],
                    },
                ),
                ("/heading", node(HEADING, &[EDITABLE, FOCUSED], vec![])),
                ("/p1", node(PARAGRAPH, &[EDITABLE, FOCUSED], vec![])),
            ],
        ),
        (
            "focused but not editable",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/p1")],
                    },
                ),
                ("/p1", node(PARAGRAPH, &[FOCUSED], vec![])),
            ],
        ),
        (
            "focused read-only paragraph",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/p1")],
                    },
                ),
                (
                    "/p1",
                    node(PARAGRAPH, &[EDITABLE, FOCUSED, READ_ONLY], vec![]),
                ),
            ],
        ),
        (
            "missing EditableText interface",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/p1")],
                    },
                ),
                (
                    "/p1",
                    DocNode {
                        role: PARAGRAPH,
                        states: Some(bits(&[EDITABLE, FOCUSED])),
                        interfaces: Some(vec![TEXT_INTERFACE]),
                        children: vec![],
                    },
                ),
            ],
        ),
        (
            "focused label without editable state",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/label")],
                    },
                ),
                ("/label", node(LABEL, &[FOCUSED], vec![])),
            ],
        ),
        (
            "unreadable descendant states",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/p1")],
                    },
                ),
                (
                    "/p1",
                    DocNode {
                        role: PARAGRAPH,
                        states: None,
                        interfaces: Some(vec![TEXT_INTERFACE, EDITABLE_TEXT]),
                        children: vec![],
                    },
                ),
            ],
        ),
        (
            "unreadable descendant interfaces beside a focused sibling",
            vec![
                (
                    "/document",
                    DocNode {
                        role: DOCUMENT_FRAME,
                        states: Some(bits(&[EDITABLE])),
                        interfaces: Some(vec![]),
                        children: vec![(OWNER, "/known"), (OWNER, "/unknown")],
                    },
                ),
                ("/known", node(PARAGRAPH, &[EDITABLE, FOCUSED], vec![])),
                (
                    "/unknown",
                    DocNode {
                        role: PARAGRAPH,
                        states: Some(bits(&[EDITABLE, FOCUSED])),
                        interfaces: None,
                        children: vec![],
                    },
                ),
            ],
        ),
    ];

    for (label, nodes) in cases {
        let (client, _server, reads) = serve(nodes);
        let root = doc_root();
        let error = zbus::block_on(focused_document_descendant(&client, &root, DOCUMENT_FRAME))
            .expect_err(label);
        assert_eq!(error.code, ErrorCode::ActionNotSupported, "{label}");
        assert_eq!(
            error.disposition,
            DeliverySemantics::not_delivered(),
            "{label}"
        );
        assert!(!zbus::block_on(focused(&client, &root)).unwrap(), "{label}");
        assert_eq!(reads.load(Ordering::SeqCst), 0, "{label}");
    }
}

#[test]
fn spreadsheet_and_presentation_documents_and_cycles_route_cleanly() {
    for role in [92u32, 93] {
        let (client, _server, _reads) = serve(vec![
            (
                "/document",
                DocNode {
                    role,
                    states: Some(bits(&[EDITABLE])),
                    interfaces: Some(vec![]),
                    children: vec![(OWNER, "/group"), ("org.other.App", "/foreign")],
                },
            ),
            (
                "/group",
                DocNode {
                    role: 39,
                    states: Some(0),
                    interfaces: Some(vec![]),
                    children: vec![(OWNER, "/document"), (OWNER, "/cell")],
                },
            ),
            ("/cell", node(56, &[EDITABLE, FOCUSED], vec![])),
        ]);
        let root = doc_root();
        let editor = zbus::block_on(focused_document_descendant(&client, &root, role)).unwrap();
        assert_eq!(editor.path(), "/cell");
        assert_eq!(editor.resolved_role(), Some("cell"));
    }
}

#[test]
fn only_document_handles_need_an_extra_routing_read() {
    assert!(needs_document_routing(Some("document")));
    assert!(!needs_document_routing(Some("paragraph")));
    assert!(!needs_document_routing(Some("heading")));
    assert!(!needs_document_routing(Some("textfield")));
    assert!(!needs_document_routing(None));
}
