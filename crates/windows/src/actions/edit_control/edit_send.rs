//! Sends the messages of an edit text replacement on Windows.
//!
//! `EM_SETSEL` and `EM_GETLIMITTEXT` carry no pointer and go through
//! `SendMessageTimeoutW` with `SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT`, which
//! also handles messages sent to the caller's own windows while it waits.
//! `EM_REPLACESEL` carries a pointer to its text, and where the edit lives
//! decides how long that text must stay allocated (`EditHome`).

use super::edit_pending::{MAX_PENDING_REPLACEMENTS, PendingPermit, PendingReplacements};
use super::edit_replace::{EditMessenger, EditSendFailure};
use super::edit_route::{EditHome, ThreadIdentity};
use agent_desktop_core::Deadline;
use std::ffi::c_void;
use std::sync::Arc;
use std::thread::Builder;
use std::time::Instant;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcessId, GetCurrentThreadId, SetEvent, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowThreadProcessId, MSG, MsgWaitForMultipleObjects, PM_NOREMOVE, PM_QS_SENDMESSAGE,
    PeekMessageW, QS_SENDMESSAGE, SMTO_ABORTIFHUNG, SMTO_ERRORONEXIT, SendMessageTimeoutW,
    SendMessageW,
};

pub(super) const EM_SETSEL: u32 = 0x00B1;
pub(super) const EM_REPLACESEL: u32 = 0x00C2;
pub(super) const EM_GETLIMITTEXT: u32 = 0x00D5;
const SEND_TIMEOUT_CAP_MS: u64 = 2_000;
const WORKER_STACK_BYTES: usize = 64 * 1024;

static PENDING_REPLACEMENTS: PendingReplacements =
    PendingReplacements::new(MAX_PENDING_REPLACEMENTS);

/// The messenger a value write uses on a real edit window.
pub(super) struct WindowMessenger;

impl EditMessenger for WindowMessenger {
    type Reservation = ReplaceRoute;

    fn reserve(&self, handle: isize) -> Result<ReplaceRoute, EditSendFailure> {
        reserve_route(handle, &PENDING_REPLACEMENTS)
    }

    fn select_all(&self, handle: isize, deadline: Deadline) -> Result<(), EditSendFailure> {
        send(handle, EM_SETSEL, 0, -1, deadline)
            .map(|_| ())
            .ok_or_else(|| EditSendFailure::Unanswered(os_detail()))
    }

    fn replace_selection(
        &self,
        route: ReplaceRoute,
        text: Vec<u16>,
        deadline: Deadline,
    ) -> Result<(), EditSendFailure> {
        send_replace(route, text, deadline)
    }
}

/// How an `EM_REPLACESEL` reaches its edit.
#[derive(Debug)]
pub(super) enum ReplaceRoute {
    /// Sent from the caller with a buffer freed when the send returns.
    Inline(isize),
    /// Sent from a worker thread that owns the buffer, holding a slot.
    Worker(isize, PendingPermit),
}

/// Picks the route for a replace to `handle`, taking a `pool` slot when
/// the edit belongs to another thread of this process.
pub(super) fn reserve_route(
    handle: isize,
    pool: &'static PendingReplacements,
) -> Result<ReplaceRoute, EditSendFailure> {
    let home = EditHome::of(window_identity(handle), caller_identity());
    if !home.needs_owned_text() {
        return Ok(ReplaceRoute::Inline(handle));
    }
    pool.try_acquire()
        .map(|permit| ReplaceRoute::Worker(handle, permit))
        .ok_or_else(|| {
            EditSendFailure::NotSent(format!(
                "{} earlier replacements are still waiting for edit controls of this process",
                pool.capacity()
            ))
        })
}

/// Sends `EM_REPLACESEL` with `text` along `route`.
///
/// Inline, the text is freed when the send returns. A window of the
/// calling thread runs its window procedure inside the send, so it reads
/// the buffer only during the call. For a window of another process the
/// system marshals the text into that process, which cannot read this
/// buffer itself. A window of another
/// thread in this process reads the buffer in place whenever its thread
/// processes the message, which can be after a timed-out send returned, so
/// a worker thread owns the buffer and sends with a blocking `SendMessageW`.
/// That call returns only once the edit's thread has processed the message,
/// or once that thread has exited and its queue holding the message is
/// gone, so no read of the buffer can follow its return, and the worker
/// frees it then. The worker holds its pool slot for its whole life, which
/// bounds the buffers and threads kept for unresponsive edits. The caller
/// waits up to the deadline and handles messages sent to its own windows
/// meanwhile, as `SendMessageTimeoutW` would.
pub(super) fn send_replace(
    route: ReplaceRoute,
    text: Vec<u16>,
    deadline: Deadline,
) -> Result<(), EditSendFailure> {
    match route {
        ReplaceRoute::Inline(handle) => {
            send(handle, EM_REPLACESEL, 1, text.as_ptr() as isize, deadline)
                .map(|_| ())
                .ok_or_else(|| EditSendFailure::Unanswered(os_detail()))
        }
        ReplaceRoute::Worker(handle, permit) => replace_on_worker(handle, text, permit, deadline),
    }
}

fn replace_on_worker(
    handle: isize,
    text: Vec<u16>,
    permit: PendingPermit,
    deadline: Deadline,
) -> Result<(), EditSendFailure> {
    let done = Completion::new()
        .map(Arc::new)
        .ok_or_else(|| EditSendFailure::NotSent(format!("CreateEventW: {}", os_error())))?;
    let signal = Arc::clone(&done);
    Builder::new()
        .name("agent-desktop-edit-replace".into())
        .stack_size(WORKER_STACK_BYTES)
        .spawn(move || {
            let text = text;
            let permit = permit;
            unsafe {
                SendMessageW(
                    handle as *mut c_void,
                    EM_REPLACESEL,
                    1,
                    text.as_ptr() as isize,
                )
            };
            drop(text);
            drop(permit);
            signal.set();
        })
        .map_err(|error| {
            EditSendFailure::NotSent(format!("the replacement thread did not start: {error}"))
        })?;
    let wait = send_timeout_ms(deadline);
    if wait_handling_sent_messages(&done, wait) {
        Ok(())
    } else {
        Err(EditSendFailure::Unanswered(format!(
            "SendMessageW unanswered after {wait} ms"
        )))
    }
}

fn wait_handling_sent_messages(done: &Completion, limit_ms: u32) -> bool {
    let start = Instant::now();
    loop {
        let elapsed = u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX);
        let Some(left) = limit_ms.checked_sub(elapsed).filter(|left| *left > 0) else {
            return done.is_set();
        };
        let woke = unsafe { MsgWaitForMultipleObjects(1, &done.0, 0, left, QS_SENDMESSAGE) };
        if woke == WAIT_OBJECT_0 {
            return true;
        }
        if woke != WAIT_OBJECT_0 + 1 {
            return done.is_set();
        }
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
}

/// A manual-reset event the worker sets once its send has returned.
struct Completion(HANDLE);

unsafe impl Send for Completion {}
unsafe impl Sync for Completion {}

impl Completion {
    fn new() -> Option<Self> {
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        (!event.is_null()).then_some(Self(event))
    }

    fn set(&self) {
        unsafe { SetEvent(self.0) };
    }

    fn is_set(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn send(
    handle: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    deadline: Deadline,
) -> Option<usize> {
    let mut answer: usize = 0;
    let sent = unsafe {
        SendMessageTimeoutW(
            handle as *mut c_void,
            message,
            wparam,
            lparam,
            SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
            send_timeout_ms(deadline),
            &mut answer,
        )
    };
    (sent != 0).then_some(answer)
}

fn send_timeout_ms(deadline: Deadline) -> u32 {
    let timeout = deadline
        .remaining()
        .as_millis()
        .clamp(1, u128::from(SEND_TIMEOUT_CAP_MS));
    u32::try_from(timeout).unwrap_or(u32::MAX)
}

fn window_identity(handle: isize) -> ThreadIdentity {
    let mut process = 0u32;
    let thread = unsafe { GetWindowThreadProcessId(handle as *mut c_void, &mut process) };
    ThreadIdentity { thread, process }
}

fn caller_identity() -> ThreadIdentity {
    ThreadIdentity {
        thread: unsafe { GetCurrentThreadId() },
        process: unsafe { GetCurrentProcessId() },
    }
}

fn os_error() -> std::io::Error {
    std::io::Error::last_os_error()
}

fn os_detail() -> String {
    format!("SendMessageTimeoutW: {}", os_error())
}
