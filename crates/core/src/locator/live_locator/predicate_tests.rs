use super::*;
use crate::locator::live_locator::test_support::evidence;

fn name_query(name: &str, exact: bool) -> LocatorQuery {
    normalize_query(&LocatorQuery {
        identity: IdentityPredicate {
            name: Some(name.into()),
            ..Default::default()
        },
        exact,
        ..Default::default()
    })
}

fn derived(name: LocatorField<String>, text: &str) -> LocatorEvidence {
    let mut evidence = evidence("treeitem", None);
    evidence.name = name;
    evidence.ref_evidence.descriptors.derived_name = Some(text.into());
    evidence.ref_evidence.descriptors.name_source = Some(crate::NameSource::ChildFieldValue);
    evidence
}

fn verdict(query: &LocatorQuery, evidence: &LocatorEvidence) -> MatchVerdict {
    self_verdict(query, evidence, &mut LocatorIdentifierStats::default())
}

#[test]
fn a_name_query_matches_a_derived_name_beside_an_absent_accessible_name() {
    let row = derived(LocatorField::Absent, "Quarterly Notes.txt");

    assert_eq!(
        verdict(&name_query("quarterly", false), &row),
        MatchVerdict::Match
    );
    assert_eq!(
        verdict(&name_query("Quarterly Notes.txt", true), &row),
        MatchVerdict::Match
    );
    assert_eq!(
        verdict(&name_query("other", false), &row),
        MatchVerdict::NoMatch
    );
}

#[test]
fn a_read_accessible_name_or_an_unread_one_outranks_the_derived_name() {
    let named = derived(LocatorField::Known("Row label".into()), "file.txt");
    let unread = derived(LocatorField::Unknown, "file.txt");

    assert_eq!(
        verdict(&name_query("file", false), &named),
        MatchVerdict::NoMatch
    );
    assert_eq!(
        verdict(&name_query("row", false), &named),
        MatchVerdict::Match
    );
    assert_eq!(
        verdict(&name_query("file", false), &unread),
        MatchVerdict::Unknown
    );
}

#[test]
fn a_text_query_matches_a_derived_name_only_beside_an_absent_accessible_name() {
    let block = derived(LocatorField::Absent, "Conceived in the late 1980s by Ada");
    let named = derived(LocatorField::Known("Row label".into()), "late 1980s");
    let text = |value: &str| search_text::normalize(value);

    assert_eq!(
        self_text_verdict(Some(&text("1980s by Ada")), &block, false),
        MatchVerdict::Match
    );
    assert_eq!(
        self_text_verdict(Some(&text("other")), &block, false),
        MatchVerdict::NoMatch
    );
    assert_eq!(
        self_text_verdict(Some(&text("1980s")), &named, false),
        MatchVerdict::NoMatch
    );
}
