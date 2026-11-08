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
/// saw is certain, and an entry whose predicate matches another element alone
/// names that element, not its own, so both are `Ambiguous`.
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
    assert_eq!(verdicts(swapped, &observed), [Ambiguous, Unknown]);
}
