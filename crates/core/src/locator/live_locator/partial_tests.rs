use super::publish_observed_matches;
use crate::locator::live_locator::{
    LocatorMatch, LocatorMatchData, LocatorResolution, LocatorResolutionMeta, LocatorStats,
};

fn observed(name: &str, token: Option<&str>, fully_observed: bool) -> LocatorMatch {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some(name));
    entry.identity.retained_object = token.map(String::from);
    entry.identity.unique_at_capture = false;
    LocatorMatch {
        data: LocatorMatchData {
            ref_id: Some("@e9".into()),
            role: "button".into(),
            name: name.into(),
            value: None,
            states: Vec::new(),
            interactive: true,
            path: Vec::new(),
        },
        document_order: 0,
        entry,
        screen_bounds: None,
        fully_observed,
    }
}

fn incomplete(matches: Vec<LocatorMatch>) -> LocatorResolution {
    LocatorResolution {
        matches,
        refmap: None,
        stats: LocatorStats::default(),
        meta: LocatorResolutionMeta {
            total_matches: 3,
            complete: false,
            selection_complete: false,
            truncated: false,
            roles_present: Vec::new(),
        },
    }
}

#[test]
fn an_incomplete_walk_publishes_refs_only_for_element_tokens_it_read_whole() {
    let mut resolution = incomplete(vec![
        observed("Cancel", Some("axtok1:cancel"), true),
        observed("Save", None, true),
        observed("Where:", Some("axtok1:where"), false),
    ]);

    publish_observed_matches(&mut resolution).unwrap();

    let refs: Vec<_> = resolution
        .matches
        .iter()
        .map(|found| found.data.ref_id.is_some())
        .collect();
    assert_eq!(
        refs,
        [true, false, false],
        "no capture proof covers an incomplete walk, so only a token may publish a ref"
    );
    assert_eq!(
        resolution.refmap.as_ref().map(crate::refs::RefMap::len),
        Some(1)
    );
}

#[test]
fn an_incomplete_walk_without_tokens_publishes_no_refmap() {
    let mut resolution = incomplete(vec![observed("Save", None, true)]);

    publish_observed_matches(&mut resolution).unwrap();

    assert!(resolution.matches[0].data.ref_id.is_none());
    assert!(resolution.refmap.is_none());
}
