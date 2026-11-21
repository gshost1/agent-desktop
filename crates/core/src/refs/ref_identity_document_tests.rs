use std::sync::Mutex;

use crate::{
    AccessibilityNode, Action, ActionRequest, ActionResult, ActionStep, AdapterError, Deadline,
    LiveElement, NativeHandle, ObservationRequest, ObservationRoot, ObservedTree, Rect, WindowInfo,
    actions::capability,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps, TreeOptions, WindowFilter},
    commands::{
        type_text::{self, TypeArgs},
        wait::{self, WaitArgs, WaitModeArgs, WaitPredicateArgs},
    },
    context::CommandContext,
    refs::refs_test_support::HomeGuard,
    snapshot::{self, SnapshotTarget},
};

struct DocumentState {
    doc_value: Option<String>,
    heading_text: String,
    paragraph_text: String,
    typed_targets: Vec<(String, String)>,
}

struct DocumentAdapter {
    window: WindowInfo,
    state: Mutex<DocumentState>,
    always_read_values: bool,
}

impl DocumentAdapter {
    fn new() -> Self {
        Self {
            window: WindowInfo {
                id: "w-doc".into(),
                title: "Draft - Writer".into(),
                app: "writer".into(),
                pid: crate::ProcessId::new(42),
                bounds: Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1000.0,
                    height: 800.0,
                }),
                state: crate::WindowState {
                    is_focused: true,
                    ..Default::default()
                },
                process_instance: Some("writer-42".into()),
            },
            state: Mutex::new(DocumentState {
                doc_value: Some("Initial document text".into()),
                heading_text: "Quarterly Heading".into(),
                paragraph_text: "Initial paragraph".into(),
                typed_targets: Vec::new(),
            }),
            always_read_values: false,
        }
    }

    fn tree(&self, text_content: bool) -> AccessibilityNode {
        let text_content = text_content || self.always_read_values;
        let state = self.state.lock().unwrap();
        let mut heading = crate::adapter::minimal_accessibility_node("heading", "Title Heading");
        heading.identity.value = text_content.then(|| state.heading_text.clone());
        heading.presentation.bounds = Some(Rect {
            x: 20.0,
            y: 40.0,
            width: 600.0,
            height: 28.0,
        });
        heading.presentation.available_actions = vec![
            capability::SET_VALUE.into(),
            capability::TYPE_TEXT.into(),
            capability::SET_FOCUS.into(),
        ];

        let mut paragraph =
            crate::adapter::minimal_accessibility_node("paragraph", "Body Paragraph");
        paragraph.identity.value = text_content.then(|| state.paragraph_text.clone());
        paragraph.presentation.bounds = Some(Rect {
            x: 20.0,
            y: 80.0,
            width: 600.0,
            height: 24.0,
        });
        paragraph.presentation.available_actions = vec![
            capability::SET_VALUE.into(),
            capability::TYPE_TEXT.into(),
            capability::SET_FOCUS.into(),
        ];

        let mut document =
            crate::adapter::minimal_accessibility_node("document", "Draft - Writer Document");
        document.identity.value = text_content.then(|| state.doc_value.clone()).flatten();
        document.presentation.bounds = Some(Rect {
            x: 10.0,
            y: 30.0,
            width: 800.0,
            height: 700.0,
        });
        document.presentation.available_actions =
            vec![capability::SELECT.into(), capability::TYPE_TEXT.into()];
        document.children = vec![heading, paragraph];

        let mut root = crate::adapter::minimal_accessibility_node("window", &self.window.title);
        root.presentation.bounds = self.window.bounds;
        root.children = vec![document];
        root
    }
}

impl ObservationOps for DocumentAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![self.window.clone()])
    }

    fn list_apps_scoped(
        &self,
        _name: &str,
        _bundle_id: Option<&str>,
        _deadline: Deadline,
    ) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![crate::AppInfo {
            name: self.window.app.clone(),
            pid: self.window.pid,
            bundle_id: None,
            process_instance: self.window.process_instance.clone(),
            presentation: None,
        }])
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let text_content = request.observation_mode.text_content;
        match root {
            ObservationRoot::Window(_) => {
                crate::adapter::observed_tree(&root, self.tree(text_content))
            }
            ObservationRoot::Element { handle, .. } => {
                let target = handle.downcast_ref::<String>().expect("handle");
                let node = match target.as_str() {
                    "document" => self.tree(text_content).children.remove(0),
                    "heading" => self.tree(text_content).children[0].children[0].clone(),
                    "paragraph" => self.tree(text_content).children[0].children[1].clone(),
                    other => panic!("unexpected element handle {other}"),
                };
                crate::adapter::observed_tree(&root, node)
            }
            ObservationRoot::Application { .. } => {
                Err(AdapterError::not_supported("application observation"))
            }
        }
    }

    fn resolve_locator_anchor(
        &self,
        entry: &crate::RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        use crate::LocatorField::{Absent, Known};

        let tree = self.tree(false);
        let node = match entry.identity.role.as_str() {
            "document" => &tree.children[0],
            "paragraph" => &tree.children[0].children[1],
            "heading" => &tree.children[0].children[0],
            role => panic!("unexpected anchor role {role}"),
        };
        let identity = super::identity_match(
            entry,
            &node.identity.name.clone().map_or(Absent, Known),
            &node.identity.value.clone().map_or(Absent, Known),
            &Absent,
            &crate::IdentifierEvidence::absent(),
        );
        if identity != crate::IdentityMatch::Match {
            return Err(AdapterError::new(
                crate::ErrorCode::StaleRef,
                "anchor identity changed",
            ));
        }
        Ok(NativeHandle::new(entry.identity.role.clone()))
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        let role = handle.downcast_ref::<String>().expect("handle");
        let node = match role.as_str() {
            "document" => self.tree(false).children.remove(0),
            "heading" => self.tree(false).children[0].children[0].clone(),
            "paragraph" => self.tree(false).children[0].children[1].clone(),
            other => panic!("unexpected handle {other}"),
        };
        Ok(LiveElement {
            identity: crate::adapter::live_identity(node.identity.name.as_deref().unwrap()),
            state: crate::ElementState {
                role: node.role,
                states: Vec::new(),
                value: node.identity.value,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            bounds: node.presentation.bounds,
            states_complete: true,
            available_actions: node.presentation.available_actions,
        })
    }
}

impl ActionOps for DocumentAdapter {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let role = handle.downcast_ref::<String>().expect("handle").clone();
        let Action::TypeText(text) = &request.action else {
            return Err(AdapterError::not_supported("only type"));
        };
        let mut state = self.state.lock().unwrap();
        state.typed_targets.push((role.clone(), text.clone()));
        if role == "document" {
            state.doc_value = Some(format!(
                "{}{}",
                state.doc_value.as_deref().unwrap_or_default(),
                text
            ));
            state.paragraph_text.push_str(text);
        } else if role == "paragraph" {
            state.paragraph_text.push_str(text);
        } else if role == "heading" {
            state.heading_text.push_str(text);
        }
        Ok(ActionResult::from_execution(
            &request.action,
            vec![ActionStep::succeeded("EditableText.InsertText")],
        ))
    }
}

impl InputOps for DocumentAdapter {}

impl SystemOps for DocumentAdapter {}

#[test]
fn consecutive_edits_on_same_document_ref_and_named_paragraph_ref_succeed_without_stale_ref() {
    let _guard = HomeGuard::new();
    let adapter = DocumentAdapter {
        always_read_values: true,
        ..DocumentAdapter::new()
    };
    let context = CommandContext::default();
    let snap = snapshot::run_with_context(
        &adapter,
        &TreeOptions::default(),
        &SnapshotTarget {
            app_name: Some("writer"),
            window_id: None,
        },
        &context,
        5_000,
    )
    .expect("snapshot succeeds");

    let doc_ref = snap.tree.children[0].ref_id.clone().expect("document ref");
    let para_ref = snap.tree.children[0].children[1]
        .ref_id
        .clone()
        .expect("named paragraph ref");

    for (ref_id, first, second) in [
        (&doc_ref, " doc-one", " doc-two"),
        (&para_ref, " para-one", " para-two"),
    ] {
        type_text::execute(
            TypeArgs {
                ref_id: ref_id.clone(),
                text: first.into(),
                timeout_ms: Some(5_000),
                interval_ms: None,
            },
            &adapter,
            &context,
        )
        .expect("first edit succeeds");
        type_text::execute(
            TypeArgs {
                ref_id: ref_id.clone(),
                text: second.into(),
                timeout_ms: Some(5_000),
                interval_ms: None,
            },
            &adapter,
            &context,
        )
        .expect("second consecutive edit on same ref must not return STALE_REF");
    }

    let state = adapter.state.lock().unwrap();
    assert!(state.paragraph_text.contains("doc-one"));
    assert!(state.paragraph_text.contains("doc-two"));
    assert!(state.paragraph_text.contains("para-one"));
    assert!(state.paragraph_text.contains("para-two"));
}

#[test]
fn wait_text_finds_text_in_paragraph_and_heading() {
    let _guard = HomeGuard::new();
    let adapter = DocumentAdapter::new();
    let context = CommandContext::default();

    for (needle, expected_role) in [
        ("Initial paragraph", "paragraph"),
        ("Quarterly Heading", "heading"),
    ] {
        let out = wait::execute(
            WaitArgs {
                mode: WaitModeArgs {
                    text: Some(needle.into()),
                    ..WaitModeArgs::default()
                },
                predicate: WaitPredicateArgs {
                    predicate: None,
                    value: None,
                    action: None,
                    count: None,
                },
                timeout_ms: 2_000,
                app: Some("writer".into()),
            },
            &adapter,
            &context,
        )
        .expect("wait --text must find document content");
        assert_eq!(out["found"], true, "{needle}");
        assert_eq!(out["role"], expected_role, "{needle}");
    }
}
