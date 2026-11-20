//! Drives `EM_REPLACESEL` routes against real edits: a held edit thread
//! leaves replacements unanswered, which must keep their text alive until
//! the late delivery, free it after, and never hold more than the pool
//! allows; a caller waiting on a worker must still answer messages sent to
//! its own windows.

use super::edit_pending::{MAX_PENDING_REPLACEMENTS, PendingReplacements};
use super::edit_replace::EditSendFailure;
use super::edit_send::{EM_SETSEL, ReplaceRoute, reserve_route, send_replace};
use super::test_support::{
    EN_CHANGE, EditOwner, SINGLE_LINE_ID, notification_count, text_of, wait_until,
};
use crate::tree::fixture::fixture_window::{destroy_window, wide};
use agent_desktop_core::Deadline;
use std::time::Duration;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, MSG, PM_NOREMOVE, PM_QS_SENDMESSAGE, PeekMessageW, SendMessageW,
};

fn deadline_ms(milliseconds: u64) -> Deadline {
    Deadline::after(milliseconds).expect("deadline")
}

fn text(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn replace(
    edit: isize,
    value: &str,
    pool: &'static PendingReplacements,
    wait_ms: u64,
) -> Result<(), EditSendFailure> {
    let route = reserve_route(edit, pool)?;
    send_replace(route, text(value), deadline_ms(wait_ms))
}

fn calling_thread_window(class: &str) -> isize {
    let class = wide(class);
    let initial = wide("");
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            initial.as_ptr(),
            0x0080,
            -30_000,
            -30_000,
            300,
            24,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    } as isize;
    assert_ne!(window, 0, "the calling thread's window is created");
    window
}

fn handle_sent_messages() {
    let mut message: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        PeekMessageW(
            &mut message,
            std::ptr::null_mut(),
            0,
            0,
            PM_NOREMOVE | PM_QS_SENDMESSAGE,
        )
    };
}

#[test]
fn an_edit_on_another_thread_of_this_process_takes_a_worker_slot() {
    static POOL: PendingReplacements = PendingReplacements::new(1);
    let owner = EditOwner::create();
    let route = reserve_route(owner.control(SINGLE_LINE_ID), &POOL).expect("a slot is free");
    assert!(matches!(route, ReplaceRoute::Worker(..)), "{route:?}");
    assert_eq!(POOL.in_flight(), 1);
    drop(route);
    assert_eq!(POOL.in_flight(), 0);
}

#[test]
fn an_unanswered_replace_keeps_its_text_until_the_edit_reads_it_and_stays_bounded() {
    static POOL: PendingReplacements = PendingReplacements::new(2);
    let owner = EditOwner::create();
    let edit = owner.control(SINGLE_LINE_ID);
    unsafe { SendMessageW(edit as HWND, EM_SETSEL, 0, -1) };
    owner.hold();
    for value in ["first late ", "second late"] {
        let answer = replace(edit, value, &POOL, 150);
        assert!(
            matches!(answer, Err(EditSendFailure::Unanswered(_))),
            "{value}: {answer:?}"
        );
    }
    assert_eq!(POOL.in_flight(), 2, "each queued replace holds its buffer");
    let refused = replace(edit, "refused", &POOL, 150);
    assert!(
        matches!(refused, Err(EditSendFailure::NotSent(_))),
        "a replace past the bound is refused unsent: {refused:?}"
    );
    assert_eq!(POOL.in_flight(), 2);
    owner.release();
    assert!(
        wait_until(Duration::from_secs(20), || POOL.in_flight() == 0),
        "every buffer is freed once the edit has read it"
    );
    let landed = text_of(edit);
    assert!(
        landed == "first late second late" || landed == "second latefirst late ",
        "both late replacements read their whole text: {landed:?}"
    );
}

#[test]
fn a_caller_waiting_on_a_worker_answers_messages_sent_to_its_own_windows() {
    static POOL: PendingReplacements = PendingReplacements::new(1);
    let owner = EditOwner::create();
    let edit = owner.control(SINGLE_LINE_ID);
    unsafe { SendMessageW(edit as HWND, EM_SETSEL, 0, -1) };
    let caller_window = calling_thread_window("STATIC");
    owner.echo_changes_to(caller_window);
    let answer = replace(edit, "echoed", &POOL, 1_500);
    let drained = wait_until(Duration::from_secs(20), || {
        handle_sent_messages();
        POOL.in_flight() == 0
    });
    owner.echo_changes_to(0);
    destroy_window(caller_window);
    assert!(drained, "the worker finished");
    assert_eq!(
        answer,
        Ok(()),
        "the edit's owner sent to the caller's window during EM_REPLACESEL"
    );
    assert_eq!(text_of(edit), "echoed");
}

#[test]
fn a_replace_on_the_calling_threads_own_edit_answers_inline_and_holds_no_slot() {
    static POOL: PendingReplacements = PendingReplacements::new(0);
    let edit = calling_thread_window("EDIT");
    let route = reserve_route(edit, &POOL);
    let inline = matches!(route, Ok(ReplaceRoute::Inline(_)));
    let answer = route.and_then(|route| send_replace(route, text("inline"), deadline_ms(150)));
    let landed = text_of(edit);
    destroy_window(edit);
    assert!(inline, "a pool without slots is never asked");
    assert_eq!(answer, Ok(()));
    assert_eq!(landed, "inline");
}

#[test]
fn an_edit_owner_that_exits_without_reading_returns_all_eight_worker_slots() {
    static POOL: PendingReplacements = PendingReplacements::new(MAX_PENDING_REPLACEMENTS);
    let mut owner = EditOwner::create();
    let edit = owner.control(SINGLE_LINE_ID);
    let changes = notification_count(edit, EN_CHANGE);
    owner.hold();
    for _ in 0..MAX_PENDING_REPLACEMENTS {
        assert!(matches!(
            replace(edit, "never read", &POOL, 30),
            Err(EditSendFailure::Unanswered(_))
        ));
    }
    assert_eq!(POOL.in_flight(), 8);
    assert!(matches!(
        replace(edit, "refused", &POOL, 30),
        Err(EditSendFailure::NotSent(_))
    ));
    owner.end_held_thread();
    assert!(wait_until(Duration::from_secs(20), || POOL.in_flight() == 0));
    assert_eq!(
        notification_count(edit, EN_CHANGE),
        changes,
        "the owner exited without delivering queued replacements"
    );
    let permits: Vec<_> = (0..MAX_PENDING_REPLACEMENTS)
        .map(|_| POOL.try_acquire().expect("every slot returned"))
        .collect();
    assert_eq!(permits.len(), 8);
    assert!(POOL.try_acquire().is_none());
    drop(permits);
    assert_eq!(POOL.in_flight(), 0);
}
