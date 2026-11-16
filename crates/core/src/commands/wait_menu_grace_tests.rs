use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{menu_wait, wait_for_menu};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::wait::menu_wait_phase::MenuWaitPhase;
use crate::{
    AdapterError, AppInfo, Deadline, MENU_OPENER_GRACE, MenuWait, ProcessId, ProcessIdentity,
    context::CommandContext,
};

const SHORT_GRACE: Duration = Duration::from_millis(300);

fn menu_app() -> Vec<AppInfo> {
    vec![AppInfo {
        name: "MenuApp".into(),
        pid: ProcessId::new(42),
        bundle_id: None,
        process_instance: Some("test-instance".into()),
        presentation: None,
    }]
}

/// Keeps the `MenuWait` core hands the adapter, so a test reads its phase
/// on a clock of its own instead of sleeping through a real grace.
#[derive(Default)]
struct Handed(Mutex<Option<MenuWait>>);

impl Handed {
    fn keep(&self, wait: MenuWait) -> Result<(), AdapterError> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(wait);
        Ok(())
    }

    fn take(&self) -> MenuWait {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .expect("core handed the adapter a wait")
    }
}

/// An adapter that keeps the trait's default opener grace.
#[derive(Default)]
struct DefaultGrace(Handed);

impl ObservationOps for DefaultGrace {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(menu_app())
    }
}

impl ActionOps for DefaultGrace {}

impl InputOps for DefaultGrace {}

impl SystemOps for DefaultGrace {
    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        wait: MenuWait,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.0.keep(wait)
    }
}

/// An adapter that measured its menus appearing sooner and says so.
#[derive(Default)]
struct ShortGrace(Handed);

impl ObservationOps for ShortGrace {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(menu_app())
    }

    fn menu_opener_grace(&self) -> Duration {
        SHORT_GRACE
    }
}

impl ActionOps for ShortGrace {}

impl InputOps for ShortGrace {}

impl SystemOps for ShortGrace {
    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        wait: MenuWait,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.0.keep(wait)
    }
}

fn grace_until(wait: MenuWait) -> Instant {
    match wait.phase() {
        MenuWaitPhase::AwaitingOpen { until, .. } => until,
        other => panic!("a closed wait with no seed awaits the opener, not {other:?}"),
    }
}

fn handed_grace(
    adapter: &dyn crate::PlatformAdapter,
    handed: &Handed,
) -> (Instant, Instant, Instant) {
    let before = Instant::now();
    wait_for_menu(
        Some("MenuApp".into()),
        false,
        5_000,
        adapter,
        &CommandContext::default(),
    )
    .expect("the stub accepts the wait");
    let after = Instant::now();
    (before, grace_until(handed.take()), after)
}

#[test]
fn a_closed_wait_holds_for_the_grace_the_adapter_supplies() {
    let adapter = ShortGrace::default();
    let (before, until, after) = handed_grace(&adapter, &adapter.0);
    assert!(
        until >= before + SHORT_GRACE,
        "the grace ends before the adapter's value"
    );
    assert!(
        until <= after + SHORT_GRACE,
        "the grace runs past the adapter's value"
    );
}

#[test]
fn an_adapter_that_supplies_no_grace_keeps_the_default() {
    let adapter = DefaultGrace::default();
    let (before, until, after) = handed_grace(&adapter, &adapter.0);
    assert!(until >= before + MENU_OPENER_GRACE);
    assert!(until <= after + MENU_OPENER_GRACE);
    assert_eq!(adapter.menu_opener_grace(), MENU_OPENER_GRACE);
}

#[test]
fn a_closed_read_after_a_short_grace_satisfies_the_wait_where_the_default_still_holds() {
    let started = Instant::now();
    let process = ProcessIdentity::new(42u32, "test-instance");
    let mut short = menu_wait(false, &process, None, started, SHORT_GRACE);
    let mut default = menu_wait(false, &process, None, started, MENU_OPENER_GRACE);
    let inside = started + SHORT_GRACE - Duration::from_millis(1);
    assert!(
        !short.observe(false, inside),
        "a read inside the short grace counted"
    );
    assert!(short.observe(false, started + SHORT_GRACE));
    assert!(!default.observe(false, started + SHORT_GRACE));
    assert!(default.observe(false, started + MENU_OPENER_GRACE));
}
