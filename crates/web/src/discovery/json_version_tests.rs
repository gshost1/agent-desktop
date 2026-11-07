use agent_desktop_core::ErrorCode;

use super::parse_version_body;
use crate::EndpointSource;

#[test]
fn a_loopback_debugger_url_on_the_same_port_is_accepted() {
    let body = r#"{"Browser":"Chrome/144.0.0.0","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/browser/abc"}"#;
    let endpoint = parse_version_body(9222, body).unwrap();
    assert_eq!(endpoint.port(), 9222);
    assert_eq!(endpoint.source, EndpointSource::JsonVersion);
    assert_eq!(endpoint.product.as_deref(), Some("Chrome/144.0.0.0"));
    assert_eq!(endpoint.websocket_url.path(), "/devtools/browser/abc");
}

#[test]
fn a_remote_or_mismatched_debugger_url_is_refused() {
    let remote = r#"{"webSocketDebuggerUrl":"ws://192.168.1.4:9222/devtools/browser/abc"}"#;
    assert_eq!(
        parse_version_body(9222, remote).unwrap_err().code,
        ErrorCode::ActionFailed
    );
    let other_port = r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:9333/devtools/browser/abc"}"#;
    assert_eq!(
        parse_version_body(9222, other_port).unwrap_err().code,
        ErrorCode::ActionFailed
    );
    assert!(parse_version_body(9222, "{}").is_err());
    assert!(parse_version_body(9222, "<html>").is_err());
}
