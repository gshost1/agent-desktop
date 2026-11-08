use std::sync::{Arc, mpsc::channel};
use std::thread;
use std::time::Duration;

use agent_desktop_core::{DeliverySemantics, ErrorCode};

use super::abandoned;
use crate::input::plan::Plan;
use crate::input::step::Step;
use crate::input::x11::fake_sink::{FakeSink, Recorded, deadline, events, run};
use crate::input::x11::progress::Progress;
use crate::input::x11::worker::{self, Link};

#[test]
fn a_timeout_during_the_first_send_never_reports_safe_to_retry() {
    let fake = FakeSink::new();
    let sink = fake.clone();
    let (ready, resume) = fake.block_next_event();
    let progress = Arc::new(Progress::default());
    let worker_progress = Arc::clone(&progress);
    let (reports, inbox) = channel();
    let (proceed_tx, proceed) = channel();
    proceed_tx.send(true).unwrap();
    let worker = thread::spawn(move || {
        worker::work(
            sink,
            vec![Plan {
                steps: vec![Step::MoveTo { x: 10, y: 10 }],
                release_origin: None,
            }],
            &Link {
                reports,
                proceed,
                journal: None,
            },
            deadline(30_000),
            &worker_progress,
        );
    });
    ready.recv_timeout(Duration::from_secs(30)).unwrap();
    progress.cancel();
    let error = abandoned(deadline(0), &progress);
    let before_resume = events(&fake);
    resume.send(()).unwrap();
    worker.join().unwrap();
    drop(inbox);

    assert!(before_resume.is_empty());
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    let details = error.details.unwrap();
    assert_eq!(details["physical_delivery_started"], true);
    assert_eq!(details["delivered_events"], 0);
    assert_eq!(events(&fake), [Recorded::Move(10, 10)]);
}

#[test]
fn a_deadline_between_characters_reports_delivered_text_without_an_emergency_release() {
    let fake = FakeSink::new();
    let text = Plan {
        steps: vec![
            Step::Key {
                code: 38,
                down: true,
            },
            Step::Key {
                code: 38,
                down: false,
            },
            Step::Pause(Duration::from_secs(10)),
            Step::Key {
                code: 39,
                down: true,
            },
            Step::Key {
                code: 39,
                down: false,
            },
        ],
        release_origin: None,
    };

    let error = run(&fake, vec![text], &mut || Ok(()), deadline(150), None).expect_err("timed out");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("evidence");
    assert_eq!(details["delivered_events"], 2);
    assert!(details.get("emergency_release_posted").is_none());
    assert_eq!(
        events(&fake),
        [Recorded::Key(38, true), Recorded::Key(38, false)]
    );
}

#[test]
fn a_gate_refusal_while_a_modifier_is_held_reports_the_workers_release() {
    let fake = FakeSink::new();
    let hold = Plan {
        steps: vec![Step::Key {
            code: 37,
            down: true,
        }],
        release_origin: None,
    };
    let tap = Plan {
        steps: vec![
            Step::Key {
                code: 38,
                down: true,
            },
            Step::Key {
                code: 38,
                down: false,
            },
        ],
        release_origin: None,
    };
    let mut gates = 0;
    let mut gate = || {
        gates += 1;
        if gates == 1 {
            Ok(())
        } else {
            Err(agent_desktop_core::AdapterError::internal("focus moved"))
        }
    };

    let error = run(&fake, vec![hold, tap], &mut gate, deadline(5_000), None)
        .expect_err("the gate refuses the second plan");

    assert_eq!(
        events(&fake),
        [Recorded::Key(37, true), Recorded::Key(37, false)]
    );
    let details = error.details.expect("evidence");
    assert_eq!(details["delivered_events"], 1);
    assert_eq!(details["emergency_release_posted"], true);
    assert_eq!(details["emergency_release_acknowledged"], true);
}
