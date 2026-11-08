use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{menu_seed, wait_for_menu};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, AppInfo, Deadline, MenuSeed, MenuWait, ProcessId, ProcessIdentity,
    context::CommandContext,
};

const POLL: Duration = Duration::from_millis(10);

/// A platform-neutral adapter that scripts the menu reads and polls them the
/// way every adapter's `wait_for_menu` does: each whole read goes to the
/// `MenuWait` core handed it, and the wait ends when that says so.
struct ScriptedMenu {
    reads: Mutex<VecDeque<bool>>,
    read_count: Mutex<usize>,
    open_now: bool,
}

impl ScriptedMenu {
    fn new(reads: &[bool]) -> Self {
        Self {
            reads: Mutex::new(reads.iter().copied().collect()),
            read_count: Mutex::new(0),
            open_now: false,
        }
    }

    fn with_menu_open_now(mut self) -> Self {
        self.open_now = true;
        self
    }

    fn read(&self) -> bool {
        *self.read_count.lock().unwrap() += 1;
        let mut reads = self.reads.lock().unwrap();
        match reads.len() {
            0 => false,
            1 => reads[0],
            _ => reads.pop_front().unwrap_or(false),
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
            if wait.observe(self.read(), read_started) {
                return Ok(());
            }
            if deadline.is_expired() {
                return Err(deadline.timeout_error());
            }
            std::thread::sleep(deadline.remaining_slice(POLL)?);
        }
    }

    fn menu_open(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Ok(self.open_now)
    }
}

fn seeded(open: bool, process: ProcessIdentity) -> CommandContext {
    CommandContext::default().with_menu_seed(Some(MenuSeed { process, open }))
}

#[test]
fn a_closed_wait_right_after_an_opener_waits_for_a_menu_that_appears_after_the_first_read() {
    let adapter = ScriptedMenu::new(&[false, false, true, true, false]);
    let value = wait_for_menu(
        Some("MenuApp".into()),
        false,
        5_000,
        &adapter,
        &CommandContext::default(),
    )
    .expect("closes after it opened");
    assert_eq!(value["found"], true);
    assert_eq!(adapter.reads(), 5, "satisfied before the menu ever opened");
}

#[test]
fn a_closed_wait_after_a_menu_item_click_returns_on_the_first_read_without_a_hold() {
    let adapter = ScriptedMenu::new(&[false]);
    let started = Instant::now();
    wait_for_menu(
        Some("MenuApp".into()),
        false,
        5_000,
        &adapter,
        &seeded(true, process()),
    )
    .expect("menu already closed");
    assert_eq!(adapter.reads(), 1);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_closed_wait_with_no_sign_of_a_menu_ends_once_the_bounded_grace_has_passed() {
    let adapter = ScriptedMenu::new(&[false]);
    let started = Instant::now();
    wait_for_menu(
        Some("MenuApp".into()),
        false,
        400,
        &adapter,
        &CommandContext::default(),
    )
    .expect("stays closed through the grace");
    let held = started.elapsed();
    assert!(held >= Duration::from_millis(200), "{held:?}");
    assert!(held < Duration::from_millis(400), "{held:?}");
}

#[test]
fn a_seed_that_saw_no_menu_or_another_process_does_not_shorten_the_wait() {
    let other = ProcessIdentity::new(42u32, "an-earlier-instance");
    for context in [seeded(false, process()), seeded(true, other)] {
        let adapter = ScriptedMenu::new(&[false, true, false]);
        wait_for_menu(Some("MenuApp".into()), false, 5_000, &adapter, &context)
            .expect("closes after it opened");
        assert_eq!(adapter.reads(), 3);
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
    let adapter = ScriptedMenu::new(&[]).with_menu_open_now();
    let seed = menu_seed(Some("MenuApp"), &adapter, Deadline::after(5_000).unwrap());
    assert_eq!(
        seed,
        Some(MenuSeed {
            process: process(),
            open: true
        })
    );
    assert_eq!(
        menu_seed(Some("Missing"), &adapter, Deadline::after(5_000).unwrap()),
        None
    );
}
