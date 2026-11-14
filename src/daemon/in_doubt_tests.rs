use super::daemon_stopped_mid_command;
use agent_desktop_core::response::output::{ErrorPayload, Response};

#[test]
fn stopped_daemon_reports_uncertain_delivery_that_is_unsafe_to_retry() {
    let error = daemon_stopped_mid_command();
    let envelope =
        serde_json::to_value(Response::err("click", ErrorPayload::from_app_error(&error))).unwrap();

    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "INTERNAL");
    assert_eq!(envelope["error"]["disposition"]["delivery"], "delivery_uncertain");
    assert_eq!(envelope["error"]["disposition"]["retry"], "unsafe");
}
