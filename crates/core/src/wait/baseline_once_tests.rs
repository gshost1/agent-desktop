use std::sync::Mutex;
use std::time::Duration;

use super::BaselineOnce;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::wait::deadline::VirtualClock;
use crate::wait::window_baseline::{WINDOW_BASELINE_BUDGET, WINDOW_BASELINE_RESERVE};
use crate::{AdapterError, CommandContext, Deadline, WaitSelector, WindowFilter, WindowInfo};

#[derive(Default)]
struct CountingWindows {
    listings: Mutex<Vec<Duration>>,
}

impl ObservationOps for CountingWindows {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.listings.lock().unwrap().push(deadline.remaining());
        Ok(Vec::new())
    }
}

impl ActionOps for CountingWindows {}

impl InputOps for CountingWindows {}

impl SystemOps for CountingWindows {}

fn entry() -> crate::RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Open"));
    entry.source.source_app = Some("TestApp".into());
    entry.source.source_window_id = Some("w-main".into());
    entry.source.source_window_title = Some("Main".into());
    entry
}

fn waiting() -> CommandContext {
    CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":Width".into(),
        gone: false,
        timeout_ms: 10_000,
    }))
}

#[test]
fn retried_dispatches_list_windows_once() {
    let _clock = VirtualClock::freeze();
    let adapter = CountingWindows::default();
    let once = BaselineOnce::default();
    let deadline = Deadline::after(5_000).unwrap();

    for _ in 0..3 {
        once.take_before_dispatch(&adapter, &entry(), &waiting(), deadline);
    }

    assert_eq!(adapter.listings.lock().unwrap().len(), 1);
    assert!(once.into_baseline().is_some());
}

#[test]
fn the_listing_stays_inside_the_action_deadline_and_its_own_cap() {
    let _clock = VirtualClock::freeze();
    let adapter = CountingWindows::default();
    let once = BaselineOnce::default();

    once.take_before_dispatch(
        &adapter,
        &entry(),
        &waiting(),
        Deadline::after(5_000).unwrap(),
    );

    let listings = adapter.listings.lock().unwrap();
    assert_eq!(listings.as_slice(), [WINDOW_BASELINE_BUDGET]);
}

#[test]
fn an_action_with_little_time_left_dispatches_without_a_listing() {
    let _clock = VirtualClock::freeze();
    let adapter = CountingWindows::default();
    let once = BaselineOnce::default();
    let short = Deadline::after(400).unwrap();
    let below_reserve =
        Deadline::after(u64::try_from(WINDOW_BASELINE_RESERVE.as_millis()).unwrap() - 1).unwrap();

    once.take_before_dispatch(&adapter, &entry(), &waiting(), short);
    BaselineOnce::default().take_before_dispatch(&adapter, &entry(), &waiting(), below_reserve);

    assert!(adapter.listings.lock().unwrap().is_empty());
    assert!(short.remaining() == Duration::from_millis(400));
    assert!(once.into_baseline().is_none());
}
