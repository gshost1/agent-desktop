use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agent_desktop_core::{Action, ActionRequest, Deadline, InteractionLease};
use zbus::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

use super::execute_action;
use crate::system::bus::tests::connected;
use crate::tree::AtspiElement;
use crate::tree::live::test_support::words;
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED};

const OWNER: &str = "org.agentdesktop.DispatchDocumentFixture";

struct Accessible {
    role: u32,
    focused: bool,
    children: Vec<&'static str>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    fn get_role(&self) -> u32 {
        self.role
    }

    fn get_role_name(&self) -> &str {
        "text"
    }

    fn get_state(&self) -> Vec<u32> {
        words(if self.focused {
            &[EDITABLE, FOCUSED]
        } else {
            &[EDITABLE]
        })
    }

    fn get_interfaces(&self) -> Vec<&str> {
        if self.children.is_empty() {
            vec!["org.a11y.atspi.Text", "org.a11y.atspi.EditableText"]
        } else {
            vec!["org.a11y.atspi.Selection"]
        }
    }

    fn get_attributes(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn get_children(&self) -> Vec<(String, OwnedObjectPath)> {
        self.children
            .iter()
            .map(|path| (OWNER.into(), (*path).try_into().unwrap()))
            .collect()
    }

    #[zbus(property)]
    fn name(&self) -> &str {
        "Draft"
    }

    #[zbus(property)]
    fn description(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn child_count(&self) -> i32 {
        self.children.len() as i32
    }

    #[zbus(property)]
    fn accessible_id(&self) -> &str {
        "draft"
    }
}

struct Text(Arc<Mutex<String>>);

#[zbus::interface(name = "org.a11y.atspi.Text")]
impl Text {
    #[zbus(property)]
    fn character_count(&self) -> i32 {
        self.0.lock().unwrap().chars().count() as i32
    }

    #[zbus(property)]
    fn caret_offset(&self) -> i32 {
        self.character_count()
    }

    fn get_n_selections(&self) -> i32 {
        0
    }

    fn get_text(&self, _start: i32, _end: i32) -> String {
        self.0.lock().unwrap().clone()
    }
}

struct EditableText(Arc<Mutex<String>>);

#[zbus::interface(name = "org.a11y.atspi.EditableText")]
impl EditableText {
    fn insert_text(&self, position: i32, text: &str, length: i32) -> bool {
        let mut value = self.0.lock().unwrap();
        assert_eq!(position as usize, value.chars().count());
        assert_eq!(length as usize, text.len());
        value.push_str(text);
        true
    }
}

#[test]
fn document_ref_dispatch_inserts_into_focused_child_and_reports_real_target() {
    for (role, expected) in [(73, "paragraph"), (83, "heading")] {
        let value = Arc::new(Mutex::new("before".to_owned()));
        let (near, far) = std::os::unix::net::UnixStream::pair().unwrap();
        let (client, server) = zbus::block_on(futures_lite::future::zip(
            Builder::async_io_unix_stream(near).p2p().build(),
            Builder::async_io_unix_stream(far)
                .server(zbus::Guid::generate())
                .unwrap()
                .p2p()
                .serve_at(
                    "/document",
                    Accessible {
                        role: 94,
                        focused: false,
                        children: vec!["/child"],
                    },
                )
                .unwrap()
                .serve_at(
                    "/child",
                    Accessible {
                        role,
                        focused: true,
                        children: vec![],
                    },
                )
                .unwrap()
                .serve_at("/child", Text(value.clone()))
                .unwrap()
                .serve_at("/child", EditableText(value.clone()))
                .unwrap()
                .build(),
        ));
        let _server = server.unwrap();
        let bus = connected(client.unwrap());
        let mut element = AtspiElement::new(OWNER, "/document", 7);
        element.set_ancestry(crate::tree::element::ancestry::of_root(&AtspiElement::new(
            OWNER, "/frame", 7,
        )));
        element.set_resolved_role("document".into());
        let handle = element.into_native_handle();
        let lease = InteractionLease::guarded(Deadline::after(5_000).unwrap(), ()).unwrap();
        let result = execute_action(
            &bus,
            &handle,
            ActionRequest::headless(Action::TypeText(" added".into())),
            &lease,
        )
        .unwrap();
        assert_eq!(*value.lock().unwrap(), "before added");
        assert_eq!(
            result.details.unwrap()["text_input_target"],
            serde_json::json!({"role": expected, "path": "/child"})
        );
    }
}
