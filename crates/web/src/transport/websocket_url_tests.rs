use super::WebSocketUrl;
use agent_desktop_core::ErrorCode;

#[test]
fn accepts_each_loopback_host() {
    for url in [
        "ws://127.0.0.1:9222/devtools/browser/abc",
        "ws://localhost:9222/devtools/browser/abc",
        "ws://[::1]:9222/devtools/browser/abc",
    ] {
        let parsed = WebSocketUrl::parse(url).unwrap();
        assert_eq!(parsed.port(), 9222);
        assert_eq!(parsed.path(), "/devtools/browser/abc");
        assert_eq!(parsed.as_url(), url);
        assert!(parsed.socket_addr().ip().is_loopback());
    }
}

#[test]
fn refuses_remote_hosts_tls_credentials_and_bad_ports() {
    for url in [
        "ws://10.0.0.5:9222/devtools/browser/abc",
        "ws://example.com:9222/devtools/browser/abc",
        "ws://127.0.0.1.evil.com:9222/devtools/browser/abc",
        "wss://127.0.0.1:9222/devtools/browser/abc",
        "http://127.0.0.1:9222/devtools/browser/abc",
        "ws://user@127.0.0.1:9222/devtools/browser/abc",
        "ws://127.0.0.1:0/devtools/browser/abc",
        "ws://127.0.0.1:99999/devtools/browser/abc",
        "ws://127.0.0.1/devtools/browser/abc",
        "ws://127.0.0.1:9222",
        "ws://127.0.0.1:9222/",
    ] {
        let error = WebSocketUrl::parse(url).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgs, "{url}");
    }
}

#[test]
fn loopback_builds_a_rooted_path() {
    let url = WebSocketUrl::loopback(4100, "devtools/browser/x").unwrap();
    assert_eq!(url.as_url(), "ws://127.0.0.1:4100/devtools/browser/x");
}
