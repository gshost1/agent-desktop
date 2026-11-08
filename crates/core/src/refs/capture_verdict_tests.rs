use super::CaptureVerdict;
use crate::RefMap;

fn refmap_json(verdict: CaptureVerdict) -> (String, String) {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    entry.identity.capture = verdict;
    let mut map = RefMap::new();
    let ref_id = map.try_allocate(entry).expect("the entry allocates");
    (
        ref_id,
        map.serialize_with_size_check().expect("the map serializes"),
    )
}

fn load(json: &str, ref_id: &str) -> CaptureVerdict {
    let map: RefMap = serde_json::from_slice(json.as_bytes()).expect("the refmap loads");
    map.validate().expect("the refmap validates");
    map.get(ref_id).expect("the ref is kept").identity.capture
}

/// A refmap written before the verdict existed carries the boolean flag. It
/// still loads: `true` is the proof it recorded, and `false` or no flag at all
/// stays the refusal those refs got, because the file cannot say which other
/// verdict it was.
#[test]
fn a_refmap_with_the_legacy_flag_still_loads() {
    let (ref_id, json) = refmap_json(CaptureVerdict::Unique);
    assert!(json.contains("\"capture\":\"unique\""), "json={json}");

    let proven = json.replace("\"capture\":\"unique\"", "\"unique_at_capture\":true");
    assert_eq!(load(&proven, &ref_id), CaptureVerdict::Unique);

    let unproven = json.replace("\"capture\":\"unique\"", "\"unique_at_capture\":false");
    assert_eq!(load(&unproven, &ref_id), CaptureVerdict::Ambiguous);

    let unflagged = json
        .replace(",\"capture\":\"unique\"", "")
        .replace("\"capture\":\"unique\",", "");
    assert!(!unflagged.contains("capture"), "json={unflagged}");
    assert_eq!(load(&unflagged, &ref_id), CaptureVerdict::Ambiguous);
}

#[test]
fn every_verdict_round_trips_through_the_refmap() {
    for verdict in [
        CaptureVerdict::Unique,
        CaptureVerdict::Ambiguous,
        CaptureVerdict::Incomplete,
        CaptureVerdict::Unknown,
    ] {
        let (ref_id, json) = refmap_json(verdict);
        assert!(!json.contains("unique_at_capture"), "json={json}");
        assert_eq!(load(&json, &ref_id), verdict, "json={json}");
    }
}

#[test]
fn a_verdict_this_build_does_not_know_is_rejected() {
    let parsed: Result<CaptureVerdict, _> = serde_json::from_str("\"guessed\"");
    assert!(parsed.is_err());
    let parsed: Result<CaptureVerdict, _> = serde_json::from_str("1");
    assert!(parsed.is_err());
}

/// The verdict names live under `capture` and the boolean only under the key
/// older refmaps wrote it with, so neither format is read as the other.
#[test]
fn each_key_accepts_only_its_own_format() {
    let (ref_id, json) = refmap_json(CaptureVerdict::Unique);

    let flag_as_verdict = json.replace("\"capture\":\"unique\"", "\"capture\":true");
    assert!(serde_json::from_str::<RefMap>(&flag_as_verdict).is_err());

    let verdict_as_flag =
        json.replace("\"capture\":\"unique\"", "\"unique_at_capture\":\"unique\"");
    assert!(serde_json::from_str::<RefMap>(&verdict_as_flag).is_err());

    let both = json.replace(
        "\"capture\":\"unique\"",
        "\"capture\":\"incomplete\",\"unique_at_capture\":true",
    );
    assert_eq!(load(&both, &ref_id), CaptureVerdict::Incomplete);
}

/// An entry built without any capture judging it claims no twin: it is
/// `Unknown`, which refuses without promising a new snapshot will help.
#[test]
fn an_entry_no_capture_judged_is_unknown() {
    assert_eq!(
        crate::RefEntryIdentity::default().capture,
        CaptureVerdict::Unknown
    );
}
