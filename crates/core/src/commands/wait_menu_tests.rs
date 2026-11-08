use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{menu_seed, wait_for_menu};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, AppInfo, Deadline, ErrorCode, MenuSeed, MenuWait, ProcessId, ProcessIdentity,
    context::CommandContext,
};

const POLL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
enum Read {
    Is(bool),
    Slow(u64, bool),
}

/// A platform-neutral adapter that scripts the menu reads and polls them the
/// way every adapter's `wait_for_menu` does: each whole read goes to the
/// `MenuWait` core handed it, a read the deadline cut is skipped, and at the
/// deadline the wait is asked whether it is satisfied.
struct ScriptedMenu {
    reads: Mutex<VecDeque<Read>>,
    timeline: Option<(Instant, Duration, Duration)>,
    read_count: Mutex<usize>,
    seed_read: (u64, bool),
}

impl ScriptedMenu {
    fn new(reads: &[bool]) -> Self {
        Self::script(&reads.iter().map(|open| Read::Is(*open)).collect::<Vec<_>>())
    }

    fn script(reads: &[Read]) -> Self {
        Self {
            reads: Mutex::new(reads.iter().copied().collect()),
            timeline: None,
            read_count: Mutex::new(0),
            seed_read: (0, false),
        }
    }

    fn opening_between(opens_at_ms: u64, closes_at_ms: u64) -> Self {
        Self {
            timeline: Some((
                Instant::now(),
                Duration::from_millis(opens_at_ms),
                Duration::from_millis(closes_at_ms),
            )),
            ..Self::new(&[])
        }
    }

    fn with_seed_read(mut self, after_ms: u64, open: bool) -> Self {
        self.seed_read = (after_ms, open);
        self
    }

    fn read(&self, deadline: Deadline) -> Result<bool, AdapterError> {
        *self.read_count.lock().unwrap() += 1;
        if let Some((origin, opens, closes)) = self.timeline {
            let at = origin.elapsed();
            return Ok(at >= opens && at < closes);
        }
        let next = {
            let mut reads = self.reads.lock().unwrap();
            match reads.len() {
                0 => Read::Is(false),
                1 => reads[0],
                _ => reads.pop_front().unwrap_or(Read::Is(false)),
            }
        };
        match next {
            Read::Is(open) => Ok(open),
            Read::Slow(ms, open) => {
                std::thread::sleep(deadline.remaining().min(Duration::from_millis(ms)));
                deadline.ensure_remaining()?;
                Ok(open)
            }
        }
    }

    fn reads(&self) -> usize {
        *self.read_count.lock().unwrap()
    }
}

fn process() -> ProcessIdentity {
    ProcessIdentity::new(42u32, "test-instance")
}

impl ObservationOps for ScriptedMenu {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "MenuApp".into(),
            pid: ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
            presentation: None,
        }])
    }
}

impl ActionOps for ScriptedMenu {}

impl InputOps for ScriptedMenu {}

impl SystemOps for ScriptedMenu {
    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        mut wait: MenuWait,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        loop {
            let read_started = Instant::now();
            match self.read(deadline) {
                Ok(open) if wait.observe(open, read_started) => return Ok(()),
                Ok(_) => {}
                Err(error) if error.code == ErrorCode::Timeout => {}
                Err(error) => return Err(error),
            }
            let gap = match deadline.remaining_slice(POLL) {
                Ok(gap) if !deadline.is_expired() => gap,
                _ if wait.satisfied_at_deadline() => return Ok(()),
                _ => return Err(deadline.timeout_error()),
            };
            std::thread::sleep(gap);
        }
    }

    fn menu_open(
        &self,
        _process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        let (after_ms, open) = self.seed_read;
        std::thread::sleep(deadline.remaining().min(Duration::from_millis(after_ms)));
        deadline.ensure_remaining()?;
        Ok(open)
    }
}

fn seeded(open: bool, process: ProcessIdentity) -> CommandContext {
    CommandContext::default().with_menu_seed(Some(MenuSeed { process, open }))
}

fn wait_closed(adapter: &ScriptedMenu, timeout_ms: u64, context: &CommandContext) -> bool {
    wait_for_menu(Some("MenuApp".into()), false, timeout_ms, adapter, context).is_ok()
}

#[test]
fn a_closed_wait_right_after_an_opener_waits_for_a_menu_that_appears_after_the_first_read() {
    let adapter = ScriptedMenu::new(&[false, false, true, true, false, false]);
    assert!(wait_closed(&adapter, 5_000, &CommandContext::default()));
    assert_eq!(adapter.reads(), 6, "satisfied before the menu ever opened");
}

#[test]
fn a_short_timeout_does_not_shrink_the_grace_below_a_late_popover() {
    let adapter = ScriptedMenu::opening_between(700, 850);
    let started = Instant::now();
    assert!(wait_closed(&adapter, 1_000, &CommandContext::default()));
    assert!(
        started.elapsed() >= Duration::from_millis(850),
        "satisfied at {:?}, before the popover that opened at 700 ms closed",
        started.elapsed()
    );
}

#[test]
fn a_slow_closed_read_with_a_short_timeout_returns_closed_not_a_timeout() {
    let adapter = ScriptedMenu::script(&[Read::Is(false), Read::Slow(60_000, false)]);
    assert!(wait_closed(&adapter, 1_000, &CommandContext::default()));
}

#[test]
fn a_closed_wait_after_a_menu_item_click_returns_after_one_more_poll_without_a_hold() {
    let adapter = ScriptedMenu::new(&[false]);
    let started = Instant::now();
    assert!(wait_closed(&adapter, 5_000, &seeded(true, process())));
    assert_eq!(adapter.reads(), 2);
    assert!(
        started.elapsed() < crate::MENU_OPENER_GRACE,
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_menu_swapped_for_another_across_one_poll_does_not_satisfy_the_wait() {
    let adapter = ScriptedMenu::new(&[false, true, false, false]);
    assert!(wait_closed(&adapter, 5_000, &seeded(true, process())));
    assert_eq!(adapter.reads(), 4);
}

#[test]
fn a_closed_wait_with_no_sign_of_a_menu_ends_once_the_grace_has_passed() {
    let adapter = ScriptedMenu::new(&[false]);
    let started = Instant::now();
    assert!(wait_closed(&adapter, 5_000, &CommandContext::default()));
    let held = started.elapsed();
    assert!(held >= crate::MENU_OPENER_GRACE, "{held:?}");
    assert!(held < Duration::from_millis(5_000), "{held:?}");
}

#[test]
fn a_seed_that_saw_no_menu_or_another_process_does_not_shorten_the_wait() {
    let other = ProcessIdentity::new(42u32, "an-earlier-instance");
    for context in [seeded(false, process()), seeded(true, other)] {
        let adapter = ScriptedMenu::new(&[false, true, false, false]);
        assert!(wait_closed(&adapter, 5_000, &context));
        assert_eq!(adapter.reads(), 4);
    }
}

#[test]
fn an_open_wait_is_not_held_and_ignores_the_seed() {
    let adapter = ScriptedMenu::new(&[true]);
    wait_for_menu(
        Some("MenuApp".into()),
        true,
        5_000,
        &adapter,
        &seeded(false, process()),
    )
    .expect("open");
    assert_eq!(adapter.reads(), 1);
}

#[test]
fn the_seed_is_one_read_of_the_waited_app_before_the_action() {
    let deadline = || Deadline::after(5_000).unwrap();
    let adapter = ScriptedMenu::new(&[]).with_seed_read(0, true);
    let seed = menu_seed(Some("MenuApp"), &adapter, deadline());
    assert_eq!(
        seed,
        Some(MenuSeed {
            process: process(),
            open: true
        })
    );
    assert_eq!(menu_seed(Some("Missing"), &adapter, deadline()), None);
}

#[test]
fn a_seed_read_slower_than_half_a_second_leaves_no_seed() {
    let adapter = ScriptedMenu::new(&[]).with_seed_read(600, true);
    assert_eq!(
        menu_seed(Some("MenuApp"), &adapter, Deadline::after(5_000).unwrap()),
        None
    );
}
