//! Holds a real edit's thread so `EM_REPLACESEL` goes unanswered, and checks
//! that the replacement keeps its text alive until the late delivery, frees
//! it after, and never holds more replacements than its pool allows.

use super::replace_selection;
use super::test_support::{EditOwner, SINGLE_LINE_ID, text_of, wait_until};
use crate::actions::edit_pending::PendingReplacements;
use crate::actions::edit_replace::EditSendFailure;
use crate::tree::fixture::fixture_window::{destroy_window, wide};
use agent_desktop_core::Deadline;
use std::time::Duration;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, SendMessageW};

const EM_SETSEL: u32 = 0x00B1;
const WM_GETTEXT: u32 = 0x000D;

fn short_deadline() -> Deadline {
    Deadline::after(150).expect("deadline")
}

fn text(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn an_unanswered_replace_keeps_its_text_until_the_edit_reads_it_and_stays_bounded() {
    static POOL: PendingReplacements = PendingReplacements::new(2);
    let owner = EditOwner::create();
    let edit = owner.control(SINGLE_LINE_ID);
    unsafe { SendMessageW(edit as HWND, EM_SETSEL, 0, -1) };
    owner.hold();
    for value in ["first late ", "second late"] {
        let answer = replace_selection(edit, text(value), short_deadline(), &POOL);
        assert!(
            matches!(answer, Err(EditSendFailure::Unanswered(_))),
            "{value}: {answer:?}"
        );
    }
    assert_eq!(POOL.in_flight(), 2, "each queued replace holds its buffer");
    let refused = replace_selection(edit, text("refused"), short_deadline(), &POOL);
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
fn a_replace_on_the_calling_threads_own_edit_answers_inline_and_holds_no_slot() {
    static POOL: PendingReplacements = PendingReplacements::new(0);
    let class = wide("EDIT");
    let initial = wide("");
    let edit = unsafe {
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
    assert_ne!(edit, 0, "the edit is created");
    let answer = replace_selection(edit, text("inline"), short_deadline(), &POOL);
    let mut buffer = [0u16; 64];
    let length = unsafe {
        SendMessageW(
            edit as HWND,
            WM_GETTEXT,
            buffer.len(),
            buffer.as_mut_ptr() as isize,
        )
    };
    destroy_window(edit);
    assert_eq!(answer, Ok(()), "a pool without slots is never asked");
    assert_eq!(
        String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)]),
        "inline"
    );
}
