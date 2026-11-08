use super::tests::{named, observed_from, verdicts, window};
use crate::CaptureVerdict;
use crate::locator::live_locator::LocatorField;

/// Each way a capture fails to prove a ref is recorded as itself: a twin it
/// counted, a read that did not cover the whole window, and a field another
/// element left unread, so resolution can refuse each with its own recovery.
#[test]
fn a_capture_records_why_it_could_not_prove_a_ref() {
    use CaptureVerdict::{Ambiguous, Incomplete, Unique, Unknown};
    let twinned = window(vec![
        named("button", "Send"),
        named("button", "Send"),
        named("button", "Cancel"),
    ]);
    assert_eq!(
        verdicts(twinned.clone(), &observed_from(twinned)),
        [Ambiguous, Ambiguous, Unique]
    );

    let mut hidden = named("scrollarea", "Feed");
    hidden.children_count = Some(2);
    let cut = window(vec![named("button", "Cancel"), hidden]);
    assert_eq!(verdicts(cut.clone(), &observed_from(cut)), [Incomplete]);

    let unread = window(vec![
        named("button", "Cancel"),
        named("button", "Other"),
        named("link", "Help"),
    ]);
    let mut observed = observed_from(unread.clone());
    observed.nodes[2].evidence.name = LocatorField::Unknown;
    assert_eq!(verdicts(unread, &observed), [Unknown, Unknown, Unique]);
}

/// Matches are counted before completeness is judged: a twin a partial read
/// saw is certain, so it is `Ambiguous`. An entry whose predicate matches one
/// other element and not its own has no twin, only an identity the capture
/// could not tie to its node, so it is `Unknown`.
#[test]
fn a_twin_is_ambiguous_however_much_was_read() {
    use CaptureVerdict::{Ambiguous, Incomplete, Unknown};
    let mut hidden = named("scrollarea", "Feed");
    hidden.children_count = Some(2);
    let cut = window(vec![
        named("button", "Send"),
        named("button", "Send"),
        named("button", "Cancel"),
        hidden,
    ]);
    assert_eq!(
        verdicts(cut.clone(), &observed_from(cut)),
        [Ambiguous, Ambiguous, Incomplete]
    );

    let swapped = window(vec![named("button", "Send"), named("button", "Cancel")]);
    let mut observed = observed_from(swapped.clone());
    observed.nodes[1].evidence.name = LocatorField::Known("Other".into());
    observed.nodes[2].evidence.name = LocatorField::Known("Send".into());
    assert_eq!(verdicts(swapped, &observed), [Unknown, Unknown]);
}

/// A drill whose root the whole-window read no longer holds is a stale cut a
/// fresh snapshot re-reads, so its entries are `Incomplete`, not `Unknown`.
#[test]
fn a_drill_root_the_read_does_not_hold_is_incomplete() {
    let root = window(vec![named("button", "Save")]);
    let observed = observed_from(root);
    let entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    let proof = super::CaptureProof::new(&observed).rooted_at(&[7]);
    assert_eq!(
        proof.verdict_at(&entry, &[7, 0]),
        CaptureVerdict::Incomplete
    );
}
