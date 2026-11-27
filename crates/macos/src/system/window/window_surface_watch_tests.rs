use super::*;
use std::collections::VecDeque;

/// Runs the reads in order with a limit far beyond them, so the answer comes
/// from the sequence alone and not from how fast the host runs. Reading past
/// the script ends the settle as "no effect".
fn scripted(reads: &[Option<&'static str>]) -> Option<&'static str> {
    let mut reads: VecDeque<Option<&'static str>> = reads.iter().copied().collect();
    match settle_with(
        || {
            reads
                .pop_front()
                .ok_or_else(|| AdapterError::internal("script exhausted"))
        },
        Duration::from_secs(60),
        Deadline::after(60_000).unwrap(),
    ) {
        Ok(signal) => signal,
        Err(error) if error.message == "script exhausted" => None,
        Err(error) => panic!("{error:?}"),
    }
}

#[test]
fn a_sheet_window_seen_again_on_the_next_read_is_not_an_effect() {
    assert_eq!(scripted(&[Some(SURFACE_WINDOW_CLOSED), None, None]), None);
    assert_eq!(
        scripted(&[
            None,
            Some(SURFACE_WINDOW_CLOSED),
            None,
            Some(SURFACE_WINDOW_CLOSED)
        ]),
        None
    );
}

#[test]
fn a_signal_seen_on_two_consecutive_reads_is_an_effect() {
    assert_eq!(
        scripted(&[
            None,
            Some(SURFACE_WINDOW_CLOSED),
            Some(SURFACE_WINDOW_CLOSED)
        ]),
        Some(SURFACE_WINDOW_CLOSED)
    );
}

#[test]
fn a_surface_that_never_reacts_settles_within_the_limit() {
    let started = Instant::now();
    let settled = settle_with(
        || Ok(None),
        Duration::from_millis(120),
        Deadline::standard().unwrap(),
    )
    .unwrap();
    assert_eq!(settled, None);
    assert!(started.elapsed() < Duration::from_millis(400));
}

#[test]
fn a_surface_whose_window_answers_as_its_host_is_unobservable() {
    assert_eq!(distinct_window(Some(42), Some(42)), None);
    assert_eq!(distinct_window(Some(43), Some(42)), Some(43));
    assert_eq!(distinct_window(Some(43), None), Some(43));
    assert_eq!(distinct_window(None, Some(42)), None);
}
