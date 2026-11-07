use super::ElementToken;

#[test]
fn round_trips_through_its_string_form() {
    let token = ElementToken {
        target_id: "8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C".into(),
        backend_node_id: 42,
    };
    let text = token.to_string();
    assert_eq!(text, "cdp:8F3A1C0B9E5D4F2A8B7C6D5E4F3A2B1C:42");
    assert!(ElementToken::is_cdp_token(&text));
    assert_eq!(ElementToken::parse(&text).unwrap(), token);
}

#[test]
fn rejects_malformed_tokens() {
    for token in [
        "",
        "cdp:",
        "cdp:T",
        "cdp::5",
        "cdp:T:0",
        "cdp:T:-3",
        "cdp:T:x",
        "cdp:T:U:5",
        "ax:T:5",
        "cdp:T 1:5",
    ] {
        assert!(ElementToken::parse(token).is_err(), "{token:?}");
    }
}
