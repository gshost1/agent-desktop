use std::sync::Mutex;

use crate::{
    AccessibilityNode, Action, ActionRequest, ActionResult, ActionStep, AdapterError, Deadline,
    LiveElement, NativeHandle, ObservationRequest, ObservationRoot, ObservedTree, Rect, WindowInfo,
    actions::capability,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps, TreeOptions, WindowFilter},
    commands::{
        type_text::{self, TypeTextArgs},
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
                is_focused: true,
                bundle_id: None,
                is_minimized: None,
                is_fullscreen: None,
                process_instance: Some("writer-42".into()),
            },
            state: Mutex::new(DocumentState {
                doc_value: None,
                heading_text: "Quarterly Heading".into(),
                paragraph_text: "Initial paragraph".into(),
                typed_targets: Vec::new(),
            }),
        }
    }

    fn tree(&self, text_content: bool) -> AccessibilityNode {
        let state = self.state.lock().unwrap();
        let mut heading = AccessibilityNode::new("heading");
        heading.identity.name = Some("Title Heading".into());
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

        let mut paragraph = AccessibilityNode::new("paragraph");
        paragraph.identity.name = Some("Body Paragraph".into());
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

        let mut document = AccessibilityNode::new("document");
        document.identity.name = Some("Draft - Writer Document".into());
        document.identity.value = text_content
            .then(|| state.doc_value.clone())
            .flatten();
        document.presentation.bounds = Some(Rect {
            x: 10.0,
            y: 30.0,
            width: 800.0,
            height: 700.0,
        });
        document.presentation.available_actions =
            vec![capability::SELECT.into(), capability::TYPE_TEXT.into()];
        document.children = vec![heading, paragraph];

        let mut root = AccessibilityNode::new("window");
        root.identity.name = Some(self.window.title.clone());
        root.presentation.bounds = self.window.bounds;
        root.children = vec![document];
        root
    }
}

impl ObservationOps for DocumentAdapter {
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
        let mut live = LiveElement::from_node(&node);
        live.state.enabled = Some(true);
        live.state.hidden = Some(false);
        live.state.offscreen = Some(false);
        Ok(live)
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

impl SystemOps for DocumentAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![self.window.clone()])
    }
}

#[test]
fn consecutive_edits_on_same_document_ref_and_named_paragraph_ref_succeed_without_stale_ref() {
    let _guard = HomeGuard::new();
    let adapter = DocumentAdapter::new();
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

    let doc_ref = snap.tree.children[0]
        .ref_id
        .clone()
        .expect("document ref");
    let para_ref = snap.tree.children[0].children[1]
        .ref_id
        .clone()
        .expect("named paragraph ref");

    for (ref_id, first, second) in [
        (&doc_ref, " doc-one", " doc-two"),
        (&para_ref, " para-one", " para-two"),
    ] {
        type_text::execute(
            TypeTextArgs {
                ref_id: ref_id.clone(),
                text: first.into(),
                timeout_ms: Some(5_000),
            },
            &adapter,
            &context,
        )
        .expect("first edit succeeds");
        type_text::execute(
            TypeTextArgs {
                ref_id: ref_id.clone(),
                text: second.into(),
                timeout_ms: Some(5_000),
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
