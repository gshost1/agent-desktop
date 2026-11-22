use super::*;

struct BudgetKeys<'a> {
    inner: Keys<'a>,
    until: Cell<Option<std::time::Instant>>,
}

impl BudgetKeys<'_> {
    fn check_budget(&self, deadline: Deadline) {
        assert!(deadline.remaining() <= Duration::from_millis(300));
        let until = std::time::Instant::now() + deadline.remaining();
        if let Some(first) = self.until.get() {
            assert!(until <= first + Duration::from_millis(1));
        } else {
            self.until.set(Some(until));
        }
    }
}

impl KeyWorld for BudgetKeys<'_> {
    fn lookup(&self, pid: u32) -> Lookup {
        self.inner.lookup(pid)
    }

    fn stopped(&self, pid: u32) -> bool {
        self.inner.stopped(pid)
    }

    fn active_client_pid(&self, deadline: Deadline) -> Option<u32> {
        self.check_budget(deadline);
        self.inner.active_client_pid(deadline)
    }

    fn focused_window(
        &self,
        pid: u32,
        deadline: Deadline,
    ) -> Result<Option<WindowInfo>, AdapterError> {
        self.check_budget(deadline);
        self.inner.focused_window(pid, deadline)
    }

    fn pause(&self, duration: Duration) {
        self.inner.pause(duration);
        std::thread::sleep(duration);
    }
}

#[test]
fn native_and_accessibility_rechecks_share_one_budget_across_both_delivery_gates() {
    let physical = FakeWorld::x11();
    let keys = BudgetKeys {
        inner: Keys::owning(&physical),
        until: Cell::new(None),
    };
    *keys.inner.active.borrow_mut() = VecDeque::from([None, Some(PID)]);
    press_in(Press {
        physical: &physical,
        keys: &keys,
        process: &process(),
        window: None,
        combo: &combo(),
        policy: InteractionPolicy::headless(),
        deadline: deadline(),
    })
    .unwrap();
    assert_eq!(keys.inner.read_count("pause"), 1);
    assert_eq!(keys.inner.read_count("focused"), 2);
    assert_eq!(physical.count("sent"), 1);
}

#[test]
fn a_new_active_dialog_gets_bounded_display_activation_rechecks() {
    for misses in 1..=3 {
        let physical = FakeWorld::x11();
        let keys = Keys::owning(&physical);
        *keys.active.borrow_mut() = std::iter::repeat_n(None, misses)
            .chain([Some(PID)])
            .collect();
        press(&physical, &keys, InteractionPolicy::headless()).unwrap();
        assert_eq!(physical.count("sent"), 1);
        assert_eq!(keys.read_count("pause"), misses);
        assert_eq!(keys.read_count("active"), misses + 2);
    }
}

#[test]
fn foreign_native_owners_never_read_accessibility_or_wait() {
    for focused in [
        Ok(None),
        Ok(Some(window(OTHER, "other"))),
        Err(AdapterError::not_supported("frames")),
    ] {
        let physical = FakeWorld::x11();
        let keys = Keys::owning(&physical);
        *keys.active.borrow_mut() = VecDeque::from([Some(OTHER), Some(PID)]);
        *keys.focused.borrow_mut() = VecDeque::from([focused]);
        let error = press(&physical, &keys, InteractionPolicy::headless()).unwrap_err();
        assert_eq!(error.details.unwrap()["cause"], "display_active_window");
        assert_eq!(keys.read_count("active"), 1);
        assert_eq!(keys.read_count("pause"), 0);
        assert_eq!(keys.read_count("focused"), 0);
        assert!(physical.sent_nothing());
    }
}

#[test]
fn a_missing_native_owner_gets_only_bounded_rechecks() {
    let physical = FakeWorld::x11();
    let keys = Keys::owning(&physical);
    *keys.active.borrow_mut() = VecDeque::from([None]);
    let error = press(&physical, &keys, InteractionPolicy::headless()).unwrap_err();
    assert_eq!(error.details.unwrap()["cause"], "display_active_window");
    assert_eq!(keys.read_count("active"), 4);
    assert_eq!(keys.read_count("pause"), 3);
    assert_eq!(keys.read_count("focused"), 0);
    assert!(physical.sent_nothing());
}

#[test]
fn expired_commands_never_wait_for_activation() {
    let physical = FakeWorld::x11();
    let keys = Keys::owning(&physical);
    *keys.active.borrow_mut() = VecDeque::from([None]);
    let expired = Deadline::after(1).unwrap();
    std::thread::sleep(Duration::from_millis(2));
    assert!(!display_active(&keys, PID, expired));
    assert_eq!(keys.read_count("active"), 1);
    assert_eq!(keys.read_count("pause"), 0);
}

#[test]
fn a_foreign_owner_ends_activation_rechecks_immediately() {
    let physical = FakeWorld::x11();
    let keys = Keys::owning(&physical);
    *keys.active.borrow_mut() = VecDeque::from([None, Some(OTHER), Some(PID)]);
    assert!(!display_active(&keys, PID, deadline()));
    assert_eq!(keys.read_count("active"), 2);
    assert_eq!(keys.read_count("pause"), 1);
    assert_eq!(keys.read_count("focused"), 0);
    assert!(physical.sent_nothing());
}

#[test]
fn native_activation_still_requires_an_accessibility_focus_answer() {
    for focused in [Ok(None), Err(AdapterError::not_supported("frames"))] {
        let physical = FakeWorld::x11();
        let keys = Keys::owning(&physical);
        *keys.active.borrow_mut() = VecDeque::from([None, Some(PID)]);
        *keys.focused.borrow_mut() = VecDeque::from([focused]);
        press(&physical, &keys, InteractionPolicy::headless()).unwrap_err();
        assert_eq!(keys.read_count("focused"), 1);
        assert!(physical.sent_nothing());
    }
}
