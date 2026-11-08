use super::*;

fn window(id: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Main".into(),
        app: "TextEdit".into(),
        pid: crate::ProcessId::new(42),
        process_instance: None,
        bounds: None,
        state: crate::WindowState::default(),
    }
}

fn adapter_error(result: Result<WindowInfo, AppError>) -> AdapterError {
    match result {
        Err(AppError::Adapter(error)) => error,
        other => panic!("expected an adapter error, got {other:?}"),
    }
}

#[test]
fn a_read_of_the_target_holds_the_focus() {
    let verdict = FocusCorroboration::judge("w1", Ok(Some(window("w1"))));

    assert_eq!(verdict.into_result().unwrap().id, "w1");
}

#[test]
fn a_read_of_another_window_names_it_and_leaves_delivery_unverified() {
    let verdict = FocusCorroboration::judge("w1", Ok(Some(window("w2"))));

    let error = adapter_error(verdict.into_result());

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(error.details.unwrap()["observed_window_id"], "w2");
}

#[test]
fn a_read_that_cannot_run_keeps_its_cause_and_leaves_delivery_unverified() {
    let cause = AdapterError::timeout("Operation exceeded its deadline");
    let verdict = FocusCorroboration::judge("w1", Err(cause));

    let error = adapter_error(verdict.into_result());

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}
