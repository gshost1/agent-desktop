use agent_desktop_core::{AdapterError, Deadline, ErrorCode, RetryDisposition};
use serde_json::json;

use super::verify_connection;
use crate::transport::scripted_transport::ScriptedTransport;
use crate::{BrowserEndpoint, CdpConnection, EndpointSource, WebSocketUrl};

fn endpoint(source: EndpointSource) -> BrowserEndpoint {
    BrowserEndpoint {
        websocket_url: WebSocketUrl::loopback(9222, "/devtools/browser/x").unwrap(),
        source,
    }
}

fn verify_against_silence(source: EndpointSource) -> AdapterError {
    let transport = ScriptedTransport::new().expect_silence("Browser.getVersion");
    let mut connection = CdpConnection::new(transport.boxed());
    verify_connection(
        &mut connection,
        &endpoint(source),
        Deadline::after(50).unwrap(),
    )
    .unwrap_err()
}

#[test]
fn an_answering_browser_is_verified() {
    let transport =
        ScriptedTransport::new().expect("Browser.getVersion", json!({ "product": "Chrome/144" }));
    let mut connection = CdpConnection::new(transport.boxed());

    let version = verify_connection(
        &mut connection,
        &endpoint(EndpointSource::DevToolsActivePort),
        Deadline::after(500).unwrap(),
    )
    .unwrap();

    assert_eq!(version["product"], "Chrome/144");
}

#[test]
fn a_silent_active_port_browser_reports_pending_approval() {
    let error = verify_against_silence(EndpointSource::DevToolsActivePort);
    assert_eq!(error.code, ErrorCode::PermDenied);
    let details = error.details.as_ref().unwrap();
    assert_eq!(details["kind"], "cdp_approval_pending");
    assert_eq!(details["port"], 9222);
    assert_eq!(details["retryable"], false);
    assert_eq!(details["approval_surface"], "sheet");
    let suggestion = error.suggestion.unwrap();
    assert!(suggestion.contains("Allow remote debugging"));
    assert!(suggestion.contains("--surface sheet"));
    assert!(suggestion.contains("do not retry"));
}

#[test]
fn a_pending_approval_is_not_marked_safe_to_retry() {
    let error = verify_against_silence(EndpointSource::DevToolsActivePort);
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
    assert_ne!(error.disposition.retry(), RetryDisposition::Safe);
}

#[test]
fn a_silent_port_endpoint_stays_a_timeout() {
    let error = verify_against_silence(EndpointSource::JsonVersion);
    assert_eq!(error.code, ErrorCode::Timeout);
}
