use super::*;

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
