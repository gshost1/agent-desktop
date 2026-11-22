use std::time::Duration;

use super::*;

fn spent() -> Deadline {
    let deadline = Deadline::after(1).expect("deadline");
    std::thread::sleep(Duration::from_millis(5));
    deadline
}

#[test]
fn a_walk_the_deadline_cut_short_is_the_timeout_not_an_unknown_modal_state() {
    let error = cut_short(spent());
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.expect("details")["kind"], "deadline");
}

#[test]
fn a_walk_cut_short_with_budget_left_could_not_establish_the_modal_state() {
    let error = cut_short(Deadline::after(60_000).expect("deadline"));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.details.expect("details")["cause"],
        "modal_state_unknown"
    );
}

#[test]
fn an_unanswered_read_is_the_timeout_only_once_the_deadline_has_passed() {
    let unanswered = || AdapterError::timeout("window did not answer");
    assert_eq!(unreadable(unanswered(), spent()).code, ErrorCode::Timeout);
    let live = unreadable(unanswered(), Deadline::after(60_000).expect("deadline"));
    assert_eq!(live.code, ErrorCode::ActionFailed);
    assert_eq!(
        live.details.expect("details")["cause"],
        "modal_state_unknown"
    );
}

mod reuse {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;
    use crate::tree::element::ancestry;
    use crate::tree::live::test_support::{read_of, words};
    use crate::tree::observe::node_evidence::{NodeEvidence, evidence_for};
    use crate::tree::placement::Placement;
    use crate::tree::properties::liveness::Liveness;
    use crate::tree::vocabulary::role_numbers::{DIALOG, FRAME, PUSH_BUTTON};
    use crate::tree::{Field, ObservedRead};

    struct Watch(Arc<AtomicBool>);

    impl GateWatch for Watch {
        fn quiet(&mut self, _: Deadline) -> bool {
            !self.0.load(Ordering::SeqCst)
        }
    }

    struct App {
        memo: &'static GateMemo,
        watched: bool,
        changed: Arc<AtomicBool>,
        walks: Arc<AtomicUsize>,
        in_window_modal: bool,
        dialog_window: bool,
    }

    fn app(watched: bool) -> App {
        App {
            memo: Box::leak(Box::new(Mutex::new(None))),
            watched,
            changed: Arc::new(AtomicBool::new(false)),
            walks: Arc::new(AtomicUsize::new(0)),
            in_window_modal: false,
            dialog_window: false,
        }
    }

    fn element(path: &str) -> AtspiElement {
        AtspiElement::new(":1.7", path, 7)
    }

    fn target(path: &str) -> AtspiElement {
        let mut target = element(path);
        target.set_ancestry(ancestry::of_root(&element("/frame")));
        target
    }

    impl NodeReader for App {
        fn read(&mut self, element: &AtspiElement) -> Result<NodeEvidence, AdapterError> {
            let mut read: ObservedRead = match element.path() {
                "/frame" => read_of(FRAME, "Settings", &[]),
                "/frame/dialog" | "/dialog" => {
                    let mut read = read_of(DIALOG, "Confirm", &[]);
                    read.node.traits.states = Field::Present(words(&[MODAL, SHOWING]));
                    read
                }
                _ => read_of(PUSH_BUTTON, "Row", &["click"]),
            };
            read.node.element = element.clone();
            Ok(NodeEvidence {
                evidence: evidence_for(&read, element),
                read,
                liveness: Liveness::Alive,
            })
        }

        fn native_tree(
            &mut self,
            root: &AtspiElement,
            deadline: Deadline,
        ) -> Result<crate::tree::Walk<ObservedRead>, AdapterError> {
            self.walks.fetch_add(1, Ordering::SeqCst);
            crate::tree::walk::walker::walk_levels(
                root.clone(),
                &crate::tree::WalkLimits::default(),
                deadline,
                |level| {
                    level
                        .iter()
                        .map(|element| {
                            let read = self.read(element)?.read;
                            let mut paths = Vec::new();
                            if element.path() == "/frame" {
                                paths.extend(["/frame/row", "/frame/other"]);
                                if self.in_window_modal {
                                    paths.push("/frame/dialog");
                                }
                            }
                            let children = paths
                                .into_iter()
                                .map(|path| {
                                    let mut child = self::element(path);
                                    child.set_ancestry(ancestry::child(
                                        element,
                                        read.node.traits.role.present().copied(),
                                    ));
                                    child
                                })
                                .collect();
                            Ok(crate::tree::Expansion::read(read, children))
                        })
                        .collect()
                },
                crate::tree::walk::wrapper::classify,
            )
        }

        fn top_levels(
            &mut self,
            _: &AtspiElement,
            _: Deadline,
        ) -> Result<Vec<AtspiElement>, AdapterError> {
            let mut windows = vec![element("/frame")];
            if self.dialog_window {
                windows.push(element("/dialog"));
            }
            Ok(windows)
        }

        fn gate_memo(&mut self) -> Option<&'static GateMemo> {
            Some(self.memo)
        }

        fn watch_app(&mut self, _: &AtspiElement, _: Deadline) -> Option<Box<dyn GateWatch>> {
            self.changed.store(false, Ordering::SeqCst);
            self.watched
                .then(|| Box::new(Watch(Arc::clone(&self.changed))) as Box<dyn GateWatch>)
        }

        fn stopped_app(&mut self, _: u32) -> Option<String> {
            None
        }

        fn placement(&mut self, _: &AtspiElement, _: Deadline) -> Placement {
            Placement::default()
        }

        fn global_position(&mut self, _: &AtspiElement, _: Deadline) -> bool {
            true
        }
    }

    fn gate(app: &mut App, path: &str) -> Result<(), AdapterError> {
        ensure_unblocked(app, &target(path), Deadline::standard().expect("deadline"))
    }

    fn blocked_cause(result: Result<(), AdapterError>) -> serde_json::Value {
        result.expect_err("blocked").details.expect("details")["cause"].clone()
    }

    #[test]
    fn a_quiet_application_reuses_the_unblocked_verdict_instead_of_walking_again() {
        let mut app = app(true);
        for _ in 0..4 {
            gate(&mut app, "/frame/row").expect("unblocked");
        }
        assert_eq!(app.walks.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_in_window_modal_shown_after_the_verdict_blocks_the_next_read() {
        let mut app = app(true);
        gate(&mut app, "/frame/row").expect("unblocked");
        app.in_window_modal = true;
        app.changed.store(true, Ordering::SeqCst);
        assert_eq!(
            blocked_cause(gate(&mut app, "/frame/row")),
            "blocked_by_modal_dialog"
        );
        assert_eq!(app.walks.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_new_top_level_dialog_ends_the_reuse_even_when_no_event_arrived() {
        let mut app = app(true);
        gate(&mut app, "/frame/row").expect("unblocked");
        app.dialog_window = true;
        assert_eq!(
            blocked_cause(gate(&mut app, "/frame/row")),
            "blocked_by_modal_dialog"
        );
        assert_eq!(app.walks.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn without_a_watch_every_read_walks() {
        let mut app = app(false);
        gate(&mut app, "/frame/row").expect("unblocked");
        gate(&mut app, "/frame/row").expect("unblocked");
        assert_eq!(app.walks.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_blocked_verdict_is_never_reused() {
        let mut app = app(true);
        app.dialog_window = true;
        assert_eq!(
            blocked_cause(gate(&mut app, "/frame/row")),
            "blocked_by_modal_dialog"
        );
        assert_eq!(
            blocked_cause(gate(&mut app, "/frame/row")),
            "blocked_by_modal_dialog"
        );
        assert_eq!(app.walks.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn another_target_in_the_same_window_walks_again() {
        let mut app = app(true);
        gate(&mut app, "/frame/row").expect("unblocked");
        gate(&mut app, "/frame/other").expect("unblocked");
        assert_eq!(app.walks.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_verdict_older_than_its_lifetime_walks_again() {
        let mut app = app(true);
        gate(&mut app, "/frame/row").expect("unblocked");
        std::thread::sleep(VERDICT_LIFETIME + Duration::from_millis(50));
        assert!(app.memo.lock().expect("memo").is_none());
        gate(&mut app, "/frame/row").expect("unblocked");
        assert_eq!(app.walks.load(Ordering::SeqCst), 2);
    }
}
