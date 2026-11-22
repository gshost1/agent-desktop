use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use agent_desktop_core::{Action, AdapterError, Deadline, InteractionPolicy, Point, Rect};
use zbus::connection::Builder;
use zbus::zvariant::OwnedObjectPath;

use super::target_steps;
use crate::actions::physical_target::tests::fake::{FakeWorld, deadline};
use crate::input::backend::Backend;
use crate::input::delivery::{Delivery, Gate};
use crate::input::keyboard::keymap::Keymap;
use crate::input::keyboard::keysym_step::KeysymStep;
use crate::input::physical::keyboard_focus;
use crate::input::physical::target_world::{LiveWorld, PhysicalWorld};
use crate::input::plan::Plan;
use crate::system::SharedBus;
use crate::system::bus::tests::connected;
use crate::tree::AtspiElement;
use crate::tree::live::test_support::words;
use crate::tree::vocabulary::state_bits::{EDITABLE, FOCUSED};

const OWNER: &str = "org.agentdesktop.KeyboardDocumentFixture";

struct Accessible {
    document: bool,
    walks: Arc<AtomicUsize>,
    focused: Arc<AtomicBool>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    fn get_role(&self) -> u32 {
        if self.document { 94 } else { 73 }
    }

    fn get_state(&self) -> Vec<u32> {
        words(if !self.document && self.focused.load(Ordering::SeqCst) {
            &[EDITABLE, FOCUSED]
        } else {
            &[EDITABLE]
        })
    }

    fn get_interfaces(&self) -> Vec<&str> {
        if self.document {
            vec![]
        } else {
            vec!["org.a11y.atspi.Text", "org.a11y.atspi.EditableText"]
        }
    }

    fn get_attributes(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn get_children(&self) -> Vec<(String, OwnedObjectPath)> {
        if self.document {
            self.walks.fetch_add(1, Ordering::SeqCst);
            vec![(OWNER.into(), "/child".try_into().unwrap())]
        } else {
            vec![]
        }
    }
}

struct World {
    bus: SharedBus,
    fake: FakeWorld,
    lose_focus: Arc<AtomicBool>,
    steal_after_key: bool,
}

impl PhysicalWorld for World {
    fn backend(&self) -> Result<Backend, AdapterError> {
        self.fake.backend()
    }

    fn keyboard_target(
        &self,
        element: &AtspiElement,
        deadline: Deadline,
    ) -> Result<AtspiElement, AdapterError> {
        let lease = agent_desktop_core::InteractionLease::guarded(deadline, ()).unwrap();
        LiveWorld {
            bus: &self.bus,
            lease: &lease,
        }
        .keyboard_target(element, deadline)
    }

    fn focus_target(
        &self,
        _element: &AtspiElement,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        Err(keyboard_focus::refused())
    }

    fn target_focused(
        &self,
        element: &AtspiElement,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        keyboard_focus::read(&self.bus, element, deadline)
    }

    fn keymap(&self, deadline: Deadline) -> Result<Keymap, AdapterError> {
        self.fake.keymap(deadline)
    }

    fn frame_active(
        &self,
        element: &AtspiElement,
        deadline: Deadline,
    ) -> Result<Option<bool>, AdapterError> {
        self.fake.frame_active(element, deadline)
    }

    fn bounds(
        &self,
        element: &AtspiElement,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        self.fake.bounds(element, deadline)
    }

    fn validate_pointer(
        &self,
        element: &AtspiElement,
        point: &Point,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.fake.validate_pointer(element, point, deadline)
    }

    fn send_plans(
        &self,
        plans: Vec<Plan>,
        gate: Gate<'_>,
        deadline: Deadline,
    ) -> Result<Delivery, AdapterError> {
        self.fake.send_plans(plans, gate, deadline)
    }

    fn send_keysyms(
        &self,
        chunks: &[Vec<KeysymStep>],
        gate: Gate<'_>,
        deadline: Deadline,
    ) -> Result<Delivery, AdapterError> {
        let mut delivered_events = 0;
        for step in chunks.iter().flatten().filter(|step| step.down) {
            self.fake.send_keysyms(&[vec![*step]], gate, deadline)?;
            delivered_events += 1;
            if self.steal_after_key {
                self.lose_focus.store(false, Ordering::SeqCst);
            }
        }
        Ok(Delivery { delivered_events })
    }

    fn pointer_refusal(&self, what: &'static str) -> AdapterError {
        self.fake.pointer_refusal(what)
    }

    fn pause(&self, duration: Duration) {
        self.fake.pause(duration);
    }
}

#[test]
fn document_multi_key_action_walks_once_and_rechecks_selected_focus() {
    for fake in [FakeWorld::x11(), FakeWorld::wayland()] {
        let walks = Arc::new(AtomicUsize::new(0));
        let focused = Arc::new(AtomicBool::new(true));
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
                        document: true,
                        walks: walks.clone(),
                        focused: focused.clone(),
                    },
                )
                .unwrap()
                .serve_at(
                    "/child",
                    Accessible {
                        document: false,
                        walks: walks.clone(),
                        focused: focused.clone(),
                    },
                )
                .unwrap()
                .build(),
        ));
        let _server = server.unwrap();
        let mut world = World {
            bus: connected(client.unwrap()),
            fake,
            lose_focus: focused.clone(),
            steal_after_key: false,
        };
        let mut document = AtspiElement::new(OWNER, "/document", 7);
        document.set_resolved_role("document".into());
        let action = Action::TypeText("a".repeat(65));
        let run = |world: &World, action: &Action| {
            target_steps(
                world,
                &document,
                action,
                InteractionPolicy::headed(),
                deadline(),
            )
        };
        run(&world, &Action::TypeText(String::new())).unwrap();
        assert_eq!(walks.load(Ordering::SeqCst), 0, "empty action");
        run(&world, &action).unwrap();
        assert_eq!(
            walks.load(Ordering::SeqCst),
            1,
            "one subtree walk per multi-key action"
        );
        run(&world, &action).unwrap();
        assert_eq!(
            walks.load(Ordering::SeqCst),
            2,
            "proof is not reused across actions"
        );
        focused.store(false, Ordering::SeqCst);
        let sent = world.fake.count("sent");
        run(&world, &action).unwrap_err();
        assert_eq!(
            world.fake.count("sent"),
            sent,
            "missing focus refuses before delivery"
        );
        if matches!(world.fake.backend, Backend::Wayland) {
            focused.store(true, Ordering::SeqCst);
            world.steal_after_key = true;
            run(&world, &action).unwrap_err();
            assert_eq!(
                world.fake.count("sent"),
                sent + 1,
                "focus loss stops the next key"
            );
        }
    }
}
