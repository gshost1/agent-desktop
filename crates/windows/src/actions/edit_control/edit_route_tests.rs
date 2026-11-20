use super::{EditHome, ThreadIdentity};

const CALLER: ThreadIdentity = ThreadIdentity {
    thread: 10,
    process: 100,
};

fn window(thread: u32, process: u32) -> ThreadIdentity {
    ThreadIdentity { thread, process }
}

#[test]
fn a_window_of_another_process_is_sent_inline() {
    let home = EditHome::of(window(20, 200), CALLER);
    assert_eq!(home, EditHome::OtherProcess);
    assert!(!home.needs_owned_text());
}

#[test]
fn a_window_of_another_process_on_a_matching_thread_id_is_still_another_process() {
    assert_eq!(
        EditHome::of(window(10, 200), CALLER),
        EditHome::OtherProcess
    );
}

#[test]
fn a_window_of_the_calling_thread_is_sent_inline() {
    let home = EditHome::of(window(10, 100), CALLER);
    assert_eq!(home, EditHome::CallingThread);
    assert!(!home.needs_owned_text());
}

#[test]
fn a_window_of_another_thread_in_this_process_needs_its_text_owned() {
    let home = EditHome::of(window(11, 100), CALLER);
    assert_eq!(home, EditHome::ThisProcess);
    assert!(home.needs_owned_text());
}

#[test]
fn a_window_that_is_gone_is_sent_inline_and_fails_there() {
    assert_eq!(EditHome::of(window(0, 0), CALLER), EditHome::OtherProcess);
    assert_eq!(EditHome::of(window(0, 100), CALLER), EditHome::OtherProcess);
}
