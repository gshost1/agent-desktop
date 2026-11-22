use super::*;

#[test]
fn a_new_active_dialog_gets_bounded_display_activation_rechecks() {
    for misses in 1..=3 {
        let physical = FakeWorld::x11();
        let keys = Keys::owning(&physical);
        *keys.active.borrow_mut() = std::iter::repeat_n(Some(OTHER), misses)
            .chain([Some(PID)])
            .collect();
        press(&physical, &keys, InteractionPolicy::headless()).unwrap();
        assert_eq!(physical.count("sent"), 1);
        assert_eq!(keys.read_count("pause"), misses);
        assert_eq!(keys.read_count("reports_active"), 1);
    }
}

#[test]
fn background_missing_and_unreadable_active_frames_do_not_wait() {
    for focused in [
        Ok(None),
        Ok(Some(window(OTHER, "other"))),
        Err(AdapterError::not_supported("frames")),
    ] {
        let physical = FakeWorld::x11();
        let keys = Keys::owning(&physical);
        *keys.active.borrow_mut() = VecDeque::from([None]);
        *keys.focused.borrow_mut() = VecDeque::from([focused]);
        let error = press(&physical, &keys, InteractionPolicy::headless()).unwrap_err();
        assert_eq!(error.details.unwrap()["cause"], "display_active_window");
        assert_eq!(keys.read_count("active"), 1);
        assert_eq!(keys.read_count("pause"), 0);
        assert!(physical.sent_nothing());
    }
}

#[test]
fn an_active_frame_cannot_override_a_persistently_foreign_native_owner() {
    let physical = FakeWorld::x11();
    let keys = Keys::owning(&physical);
    *keys.active.borrow_mut() = VecDeque::from([Some(OTHER)]);
    let error = press(&physical, &keys, InteractionPolicy::headless()).unwrap_err();
    assert_eq!(error.details.unwrap()["cause"], "display_active_window");
    assert_eq!(keys.read_count("active"), 4);
    assert_eq!(keys.read_count("pause"), 3);
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
    assert_eq!(keys.read_count("reports_active"), 0);
    assert_eq!(keys.read_count("pause"), 0);
}
