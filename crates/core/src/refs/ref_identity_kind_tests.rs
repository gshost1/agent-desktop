use super::*;

#[test]
fn saved_identifier_matches_either_live_identifier_source() {
    let mut entry = entry();
    entry.identity.native_id = Some(dom_identifier("dom-submit"));
    let identifiers = IdentifierEvidence::typed(
        [identifier("ax-submit"), dom_identifier("dom-submit")],
        Some(0),
        true,
    );

    assert_eq!(
        identity_match(
            &entry,
            &LocatorField::Absent,
            &LocatorField::Absent,
            &LocatorField::Absent,
            &identifiers,
        ),
        IdentityMatch::Match
    );
}

#[test]
fn saved_identifier_does_not_match_the_same_value_of_another_kind() {
    let mut entry = entry();
    entry.identity.native_id = Some(dom_identifier("submit"));
    let identifiers = IdentifierEvidence::typed([identifier("submit")], Some(0), true);

    assert_eq!(
        identity_match(
            &entry,
            &LocatorField::Absent,
            &LocatorField::Absent,
            &LocatorField::Absent,
            &identifiers,
        ),
        IdentityMatch::NoMatch
    );
}

#[test]
fn ax_only_and_dom_only_identifiers_resolve_deterministically() {
    for (make, value) in [
        (
            identifier as fn(&str) -> crate::ElementIdentifier,
            "ax-only",
        ),
        (dom_identifier, "dom-only"),
    ] {
        let mut entry = entry();
        entry.identity.native_id = Some(make(value));
        let live = IdentifierEvidence::typed([make(value)], Some(0), true);

        assert_eq!(
            identity_match(
                &entry,
                &LocatorField::Absent,
                &LocatorField::Absent,
                &LocatorField::Absent,
                &live,
            ),
            IdentityMatch::Match
        );
    }
}

#[test]
fn ax_and_devtools_tokens_are_element_tokens_and_paths_are_not() {
    assert!(is_element_token("axtok1:7f3a"));
    assert!(is_element_token("cdp:TARGET:42"));
    assert!(!is_element_token("path:0/3/1"));
    assert!(!is_element_token("cdpx:TARGET:42"));
}
