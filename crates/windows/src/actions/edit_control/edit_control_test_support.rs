//! A real Win32 owner window with classic edits and combo boxes, for tests
//! that drive them through `set_value_steps` and record what the owner is
//! told.
//!
//! The owner's window procedure logs every `WM_COMMAND` notification, so a
//! test can assert that the owner heard the change, not only that the text
//! reads back. A posted hold message parks one owner's thread inside its
//! window procedure, so no sent message reaches its windows until release.

use crate::actions::value_write::set_value_steps;
use crate::tree::automation::root_from_hwnd;
use crate::tree::fixture::fixture_window::{
    close_window, destroy_window, pump_until_destroyed, quit_pump, register_class_with_proc,
    unique_class_name, unregister_class, wide,
};
use agent_desktop_core::{Deadline, InteractionPolicy};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, sleep, spawn};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, FindWindowExW, GWLP_USERDATA, GetDlgItem, GetWindowLongPtrW,
    PostMessageW, PostQuitMessage, SW_SHOWNOACTIVATE, SendMessageW, SetWindowLongPtrW, ShowWindow,
    WM_COMMAND, WM_DESTROY, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const READY: u32 = 0x0400 + 7;
const HOLD: u32 = 0x0400 + 8;
const EM_SETLIMITTEXT: u32 = 0x00C5;
const WS_DISABLED: u32 = 0x0800_0000;
pub(super) const EN_CHANGE: u32 = 0x0300;
pub(super) const CBN_EDITCHANGE: u32 = 5;
const WM_GETTEXT: u32 = 0x000D;
pub(super) const COMBO_ID: i32 = 101;
pub(super) const MULTILINE_ID: i32 = 102;
pub(super) const SINGLE_LINE_ID: i32 = 103;
pub(super) const READ_ONLY_ID: i32 = 104;
pub(super) const DROP_LIST_ID: i32 = 105;
pub(super) const FIXED_WIDTH_ID: i32 = 106;
pub(super) const DISABLED_ID: i32 = 107;
pub(super) const LIMITED_ID: i32 = 108;
/// The text limit the `LIMITED_ID` edit is given, in UTF-16 units.
pub(super) const LIMITED_LENGTH: usize = 8;

const ECHO: u32 = 0x0400 + 9;

static NOTIFICATIONS: Mutex<Vec<(isize, u32)>> = Mutex::new(Vec::new());

/// One owner window's test controls, reached from its window procedure
/// through `GWLP_USERDATA`, so parallel tests never steer each other.
#[derive(Default)]
struct OwnerState {
    holding: AtomicBool,
    held: AtomicBool,
    echo_to: AtomicIsize,
}

fn owner_state<'a>(window: HWND) -> Option<&'a OwnerState> {
    let state = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *const OwnerState;
    unsafe { state.as_ref() }
}

unsafe extern "system" fn owner_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = owner_state(window);
    if message == WM_COMMAND {
        let code = ((wparam >> 16) & 0xFFFF) as u32;
        if let Ok(mut log) = NOTIFICATIONS.lock() {
            log.push((lparam, code));
        }
        let echo_to = state.map_or(0, |state| state.echo_to.load(Ordering::Acquire));
        if code == EN_CHANGE && echo_to != 0 {
            unsafe { SendMessageW(echo_to as HWND, ECHO, 0, 0) };
        }
    }
    if message == HOLD
        && let Some(state) = state
    {
        state.held.store(true, Ordering::Release);
        while state.holding.load(Ordering::Acquire) {
            sleep(Duration::from_millis(5));
        }
        state.held.store(false, Ordering::Release);
        return 0;
    }
    if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return 0;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

pub(super) struct EditOwner {
    window: isize,
    thread_id: u32,
    class_name: String,
    pump: Option<JoinHandle<()>>,
    state: Arc<OwnerState>,
}

impl EditOwner {
    pub(super) fn create() -> Self {
        let class_name = unique_class_name();
        let (sender, receiver) = channel();
        let state = Arc::new(OwnerState::default());
        let state_address = Arc::as_ptr(&state) as isize;
        let pump = spawn({
            let class_name = class_name.clone();
            move || {
                register_class_with_proc(&class_name, owner_proc).expect("class registers");
                let window = create_owner(&class_name);
                unsafe { SetWindowLongPtrW(window as HWND, GWLP_USERDATA, state_address) };
                unsafe { ShowWindow(window as HWND, SW_SHOWNOACTIVATE) };
                unsafe { PostMessageW(window as HWND, READY, 0, 0) };
                pump_until_destroyed(window, sender, READY);
            }
        });
        let running = receiver
            .recv_timeout(Duration::from_secs(20))
            .expect("the owner window pumps")
            .expect("the owner window exists");
        Self {
            window: running.window,
            thread_id: running.thread_id,
            class_name,
            pump: Some(pump),
            state,
        }
    }

    pub(super) fn control(&self, id: i32) -> isize {
        let handle = unsafe { GetDlgItem(self.window as HWND, id) } as isize;
        assert_ne!(handle, 0, "control {id} exists");
        handle
    }

    pub(super) fn combo_edit(&self) -> isize {
        let class = wide("Edit");
        let handle = unsafe {
            FindWindowExW(
                self.control(COMBO_ID) as HWND,
                std::ptr::null_mut(),
                class.as_ptr(),
                std::ptr::null(),
            )
        } as isize;
        assert_ne!(handle, 0, "the drop-down combo box owns an edit");
        handle
    }

    /// Parks the owner's thread inside its window procedure until
    /// `release`, so messages sent to its windows wait unanswered.
    pub(super) fn hold(&self) {
        self.state.holding.store(true, Ordering::Release);
        unsafe { PostMessageW(self.window as HWND, HOLD, 0, 0) };
        assert!(
            wait_until(Duration::from_secs(20), || {
                self.state.held.load(Ordering::Acquire)
            }),
            "the owner thread is held"
        );
    }

    pub(super) fn release(&self) {
        self.state.holding.store(false, Ordering::Release);
    }

    /// Makes the owner, on every `EN_CHANGE`, send a message to `window`
    /// and wait for its answer before the edit's change completes.
    pub(super) fn echo_changes_to(&self, window: isize) {
        self.state.echo_to.store(window, Ordering::Release);
    }
}

impl Drop for EditOwner {
    fn drop(&mut self) {
        self.release();
        close_window(self.window);
        quit_pump(self.thread_id);
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
        destroy_window(self.window);
        unregister_class(&self.class_name);
    }
}

fn create_owner(class_name: &str) -> isize {
    let name = wide(class_name);
    let title = wide("edit owner fixture");
    let window = unsafe {
        CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            -30_000,
            -30_000,
            420,
            400,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    assert!(!window.is_null(), "the owner window is created");
    child(window, "COMBOBOX", 0x0002 | 0x0040, COMBO_ID, 8);
    child(window, "EDIT", 0x0004 | 0x0040, MULTILINE_ID, 140);
    child(window, "EDIT", 0x0080, SINGLE_LINE_ID, 200);
    child(window, "EDIT", 0x0800, READ_ONLY_ID, 232);
    child(window, "COMBOBOX", 0x0003, DROP_LIST_ID, 264);
    child(window, "EDIT", 0, FIXED_WIDTH_ID, 296);
    child(window, "EDIT", 0x0080 | WS_DISABLED, DISABLED_ID, 328);
    let limited = child(window, "EDIT", 0x0080, LIMITED_ID, 360);
    unsafe { SendMessageW(limited, EM_SETLIMITTEXT, LIMITED_LENGTH, 0) };
    window as isize
}

fn child(parent: HWND, class: &str, style: u32, id: i32, top: i32) -> HWND {
    let class = wide(class);
    let text = wide("before");
    let handle = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            8,
            top,
            300,
            if id == COMBO_ID || id == DROP_LIST_ID {
                120
            } else {
                24
            },
            parent,
            id as isize as *mut c_void,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    assert!(!handle.is_null(), "control {id} is created");
    handle
}

pub(super) fn heard(control: isize, code: u32) -> bool {
    NOTIFICATIONS
        .lock()
        .map(|log| log.contains(&(control, code)))
        .unwrap_or(false)
}

pub(super) fn text_of(handle: isize) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe {
        SendMessageW(
            handle as HWND,
            WM_GETTEXT,
            buffer.len(),
            buffer.as_mut_ptr() as isize,
        )
    };
    String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)])
}

pub(super) fn deadline() -> Deadline {
    Deadline::after(10_000).expect("deadline")
}

pub(super) fn write(handle: isize, value: &str) -> Vec<agent_desktop_core::ActionStep> {
    let element = root_from_hwnd(handle, deadline()).expect("the control resolves");
    set_value_steps(&element, value, InteractionPolicy::headless(), deadline())
        .expect("set-value delivers")
}

/// Polls `condition` until it holds or `limit` passes.
pub(super) fn wait_until(limit: Duration, condition: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while !condition() {
        if start.elapsed() > limit {
            return false;
        }
        sleep(Duration::from_millis(5));
    }
    true
}
