use agent_desktop_core::ErrorCode;

use super::ElementToken;

fn token() -> ElementToken {
    ElementToken {
        target_id: "8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C".into(),
        frame_id: "8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C".into(),
        loader_id: "0D9C8B7A6F5E4D3C2B1A09F8E7D6C5B4".into(),
        backend_node_id: 42,
    }
}

#[test]
fn round_trips_through_its_string_form() {
    let text = token().to_string();
    assert_eq!(
        text,
        "cdp:8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C:8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C:0D9C8B7A6F5E4D3C2B1A09F8E7D6C5B4:42"
    );
    assert!(ElementToken::is_cdp_token(&text));
    assert_eq!(ElementToken::parse(&text).unwrap(), token());
}

#[test]
fn a_token_without_document_identity_is_stale() {
    let error = ElementToken::parse("cdp:8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C:42").unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn rejects_malformed_tokens() {
    for token in [
        "",
        "cdp:",
        "cdp:T",
        "cdp::5",
        "cdp:T:0",
        "cdp:T:F:L:0",
        "cdp:T:F:L:-3",
        "cdp:T:F:L:x",
        "cdp:T:F::5",
        "cdp:T:U:5",
        "cdp:T:F:L:M:5",
        "ax:T:F:L:5",
        "cdp:T 1:F:L:5",
    ] {
        let error = ElementToken::parse(token).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgs, "{token:?}");
    }
}
