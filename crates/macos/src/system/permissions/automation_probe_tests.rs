use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static STARTED: AtomicU32 = AtomicU32::new(0);
static CONCURRENT_STARTED: AtomicU32 = AtomicU32::new(0);
static PROMPT_ANSWERED: AtomicBool = AtomicBool::new(false);

fn granted() -> i32 {
    0
}

fn stuck_behind_a_prompt() -> i32 {
    STARTED.fetch_add(1, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(600));
    0
}

fn waits_for_the_prompt() -> i32 {
    CONCURRENT_STARTED.fetch_add(1, Ordering::SeqCst);
    while !PROMPT_ANSWERED.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(5));
    }
    0
}

fn slot() -> Mutex<Option<SharedProbe>> {
    Mutex::new(None)
}

#[test]
fn an_answering_probe_returns_its_status() {
    let status = probe_with(&slot(), Duration::from_secs(5), granted).unwrap();

    assert_eq!(status, 0);
}

#[test]
fn a_probe_blocked_behind_a_prompt_gives_up_and_the_next_call_reuses_it() {
    let slot = slot();
    let before = STARTED.load(Ordering::SeqCst);

    let first = probe_with(&slot, Duration::from_millis(50), stuck_behind_a_prompt).unwrap();
    let second = probe_with(&slot, Duration::from_millis(50), stuck_behind_a_prompt).unwrap();
    let answered = probe_with(&slot, Duration::from_secs(5), stuck_behind_a_prompt).unwrap();

    assert_eq!(first, PROBE_UNANSWERED);
    assert_eq!(second, PROBE_UNANSWERED);
    assert_eq!(answered, 0);
    assert_eq!(STARTED.load(Ordering::SeqCst) - before, 1);
}

#[test]
fn a_spent_deadline_is_a_timeout_not_an_unanswered_probe() {
    let error = probe_within(Deadline::after(0).unwrap(), granted).unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}

#[test]
fn an_unanswered_probe_is_an_unknown_permission_and_names_the_prompt() {
    assert_eq!(
        super::super::map_automation_probe(PROBE_UNANSWERED),
        agent_desktop_core::PermissionState::Unknown
    );
    let error = super::super::automation_requirement(PROBE_UNANSWERED).unwrap_err();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::AppUnresponsive);
    let details = error.details.as_ref().expect("details");
    assert_eq!(details["answered"], false);
    assert_eq!(details["retryable"], false);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("prompt"))
    );
}

#[test]
fn concurrent_callers_during_a_pending_prompt_share_one_probe() {
    let slot = Arc::new(slot());
    let mut callers = Vec::new();
    for _ in 0..8 {
        let slot = Arc::clone(&slot);
        callers.push(std::thread::spawn(move || {
            probe_with(&slot, Duration::from_millis(150), waits_for_the_prompt).unwrap()
        }));
    }
    let statuses = callers
        .into_iter()
        .map(|caller| caller.join().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(statuses, [PROBE_UNANSWERED; 8]);
    assert_eq!(
        CONCURRENT_STARTED.load(Ordering::SeqCst),
        1,
        "callers that overlap while a consent prompt blocks the probe must wait on the same \
         probe instead of each leaving a blocked thread behind"
    );

    PROMPT_ANSWERED.store(true, Ordering::SeqCst);
    let answered = probe_with(&slot, Duration::from_secs(5), waits_for_the_prompt).unwrap();

    assert_eq!(answered, 0);
    assert_eq!(CONCURRENT_STARTED.load(Ordering::SeqCst), 1);
    assert!(
        slot.lock().unwrap().is_none(),
        "an answered probe is cleared so the next call asks again"
    );
}
