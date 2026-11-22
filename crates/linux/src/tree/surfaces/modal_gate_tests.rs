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
    use crate::tree::surfaces::modal_gate_memo::GateMemo;

    use super::*;
    use crate::tree::element::ancestry;
    use crate::tree::live::test_support::{read_of, words};
    use crate::tree::observe::node_evidence::{NodeEvidence, evidence_for};
    use crate::tree::placement::Placement;
    use crate::tree::properties::liveness::Liveness;
    use crate::tree::vocabulary::role_numbers::{DIALOG, FRAME, PUSH_BUTTON};
    use crate::tree::{Field, ObservedRead};

    struct App {
        walks: usize,
        inventories: usize,
        in_window_modal: bool,
        dialog_window: bool,
    }

    fn app() -> App {
        App {
            walks: 0,
            inventories: 0,
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
            self.walks += 1;
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
            self.inventories += 1;
            let mut windows = vec![element("/frame")];
            if self.dialog_window {
                windows.push(element("/dialog"));
            }
            Ok(windows)
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

    fn gate(
        app: &mut App,
        memo: Option<&GateMemo>,
        deadline: Deadline,
        may_reuse: bool,
    ) -> Result<(), AdapterError> {
        ensure_unblocked(app, &target("/frame/row"), deadline, memo, may_reuse)
    }

    fn blocked_cause(result: Result<(), AdapterError>) -> serde_json::Value {
        result.expect_err("blocked").details.expect("details")["cause"].clone()
    }

    #[test]
    fn a_provisional_read_after_a_full_read_reuses_the_same_deadline_verdict() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        gate(&mut app, Some(&memo), deadline, true).unwrap();
        assert_eq!(app.walks, 1);
        assert_eq!(app.inventories, 2);
    }

    #[test]
    fn a_full_read_always_walks_even_with_a_matching_verdict() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        assert_eq!(app.walks, 2);
    }

    #[test]
    fn an_in_window_modal_after_a_provisional_verdict_blocks_the_next_full_read() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, true).unwrap();
        app.in_window_modal = true;
        assert_eq!(
            blocked_cause(gate(&mut app, Some(&memo), deadline, false)),
            "blocked_by_modal_dialog"
        );
        assert_eq!(app.walks, 2);
    }

    #[test]
    fn a_new_top_level_dialog_makes_a_provisional_read_walk_and_block() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        app.dialog_window = true;
        assert_eq!(
            blocked_cause(gate(&mut app, Some(&memo), deadline, true)),
            "blocked_by_modal_dialog"
        );
        assert_eq!(app.walks, 3);
    }

    #[test]
    fn another_deadline_walks_again() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        let another = Deadline::after(60_000).unwrap();
        assert_ne!(deadline, another);
        gate(&mut app, Some(&memo), another, true).unwrap();
        assert_eq!(app.walks, 2);
    }

    #[test]
    fn a_blocked_full_verdict_is_never_reused_by_a_provisional_read() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        app.dialog_window = true;
        assert_eq!(
            blocked_cause(gate(&mut app, Some(&memo), deadline, false)),
            "blocked_by_modal_dialog"
        );
        assert_eq!(
            blocked_cause(gate(&mut app, Some(&memo), deadline, true)),
            "blocked_by_modal_dialog"
        );
        assert_eq!(app.walks, 5);
    }

    #[test]
    fn another_target_in_the_same_window_walks_again() {
        let mut app = app();
        let memo = GateMemo::default();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, Some(&memo), deadline, false).unwrap();
        ensure_unblocked(
            &mut app,
            &target("/frame/other"),
            deadline,
            Some(&memo),
            true,
        )
        .unwrap();
        assert_eq!(app.walks, 2);
    }

    #[test]
    fn without_a_memo_every_read_walks() {
        let mut app = app();
        let deadline = Deadline::standard().unwrap();
        gate(&mut app, None, deadline, false).unwrap();
        gate(&mut app, None, deadline, true).unwrap();
        assert_eq!(app.walks, 2);
    }
}
