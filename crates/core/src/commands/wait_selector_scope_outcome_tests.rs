use super::*;

fn wait_through_the_command(app: &ScriptedApp) -> AdapterError {
    let entry = entry();
    let context = CommandContext::default().with_wait_selector(Some(selector(10_000)));
    let baseline = WindowBaseline::before_action(
        app,
        &entry,
        &context,
        crate::Deadline::after(5_000).unwrap(),
    );
    let delivered = serde_json::to_value(crate::ActionResult::delivered_unverified("click"));
    adapter_error(crate::commands::helpers::apply_post_action_wait(
        delivered.unwrap(),
        Some(&entry),
        baseline.as_ref(),
        app,
        &context,
    ))
}

#[test]
fn reads_that_fail_while_the_process_check_says_running_never_end_the_wait_early() {
    let _home = HomeGuard::new();
    let clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(vec![open(MAIN, 0)], (DIALOG, NEVER));
    app.unresponsive = (ms(100), NEVER);
    app.process_check_agrees = false;

    let error = adapter_error(wait_after_action(&app, 10_000));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(clock.elapsed() >= ms(10_000), "{:?}", clock.elapsed());
}

#[test]
fn a_single_window_not_found_read_does_not_widen_the_scope() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let flicker = Window {
        id: MAIN,
        opens: Duration::ZERO,
        closes: ms(250),
    };
    let back = Window {
        id: MAIN,
        opens: ms(330),
        closes: NEVER,
    };
    let app = ScriptedApp::new(vec![flicker, back, open(OTHER, 0)], (OTHER, ms(0)));

    let error = adapter_error(wait_after_action(&app, 2_000));

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn a_gone_process_keeps_the_action_disposition_and_report() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(vec![open(MAIN, 0)], (DIALOG, NEVER));
    app.exits = ms(300);

    let error = wait_through_the_command(&app);

    assert_eq!(error.code, ErrorCode::AppNotFound);
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "wait_target_gone");
    assert_eq!(details["after_action"]["action"], "click");
}

#[test]
fn a_confirmed_unresponsive_owner_keeps_the_action_disposition_and_report() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let mut app = ScriptedApp::new(vec![open(MAIN, 0)], (DIALOG, NEVER));
    app.unresponsive = (ms(100), NEVER);

    let error = wait_through_the_command(&app);

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "wait_owner_unresponsive");
    assert_eq!(details["after_action"]["action"], "click");
}

#[test]
fn a_gone_wait_keeps_the_ref_window_scope() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let app = ScriptedApp::new(vec![open(MAIN, 0), open(DIALOG, 50)], (MAIN, ms(100)));
    let entry = entry();
    let baseline = WindowBaseline::capture(
        &app,
        "TestApp",
        entry.process.identity().unwrap(),
        crate::Deadline::after(500).unwrap(),
    );

    let result = execute_after_action(
        WaitSelectorInput {
            query_raw: "button:Cancel".into(),
            gone: true,
            app: Some("TestApp".into()),
            window_id: Some(MAIN.into()),
            opts: crate::adapter::TreeOptions::default(),
            timeout_ms: 2_000,
        },
        &entry,
        baseline.as_ref(),
        &app,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(result["target_absent"], true);
}
