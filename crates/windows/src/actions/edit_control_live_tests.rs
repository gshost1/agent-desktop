//! Drives real Win32 edits and combo boxes through `set_value_steps` and
//! records what their owner window is told.
//!
//! The owner's window procedure logs every `WM_COMMAND` notification, so a
//! test can assert that the owner heard the change, not only that the text
//! reads back.

use super::{EDIT_LABEL, EditTarget, edit_target};
use crate::actions::value_write::{clear_steps, set_value_steps};
use crate::tree::automation::root_from_hwnd;
use crate::tree::fixture::ensure_test_apartment;
use crate::tree::fixture::fixture_window::{
    close_window, destroy_window, pump_until_destroyed, quit_pump, register_class_with_proc,
    unique_class_name, unregister_class, wide,
};
use agent_desktop_core::{Deadline, InteractionPolicy};
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::mpsc::channel;
use std::thread::{JoinHandle, spawn};
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, FindWindowExW, GetDlgItem, PostMessageW, PostQuitMessage,
    SW_SHOWNOACTIVATE, SendMessageW, ShowWindow, WM_COMMAND, WM_DESTROY, WS_CHILD,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const READY: u32 = 0x0400 + 7;
const EN_CHANGE: u32 = 0x0300;
const CBN_EDITCHANGE: u32 = 5;
const WM_GETTEXT: u32 = 0x000D;
const COMBO_ID: i32 = 101;
const MULTILINE_ID: i32 = 102;
const SINGLE_LINE_ID: i32 = 103;
const READ_ONLY_ID: i32 = 104;
const DROP_LIST_ID: i32 = 105;

static NOTIFICATIONS: Mutex<Vec<(isize, u32)>> = Mutex::new(Vec::new());

unsafe extern "system" fn owner_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_COMMAND
        && let Ok(mut log) = NOTIFICATIONS.lock()
    {
        log.push((lparam, ((wparam >> 16) & 0xFFFF) as u32));
    }
    if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return 0;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

struct EditOwner {
    window: isize,
    thread_id: u32,
    class_name: String,
    pump: Option<JoinHandle<()>>,
}

impl EditOwner {
    fn create() -> Self {
        let class_name = unique_class_name();
        let (sender, receiver) = channel();
        let pump = spawn({
            let class_name = class_name.clone();
            move || {
                register_class_with_proc(&class_name, owner_proc).expect("class registers");
                let window = create_owner(&class_name);
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
        }
    }

    fn control(&self, id: i32) -> isize {
        let handle = unsafe { GetDlgItem(self.window as HWND, id) } as isize;
        assert_ne!(handle, 0, "control {id} exists");
        handle
    }

    fn combo_edit(&self) -> isize {
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
}

impl Drop for EditOwner {
    fn drop(&mut self) {
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
            320,
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
    window as isize
}

fn child(parent: HWND, class: &str, style: u32, id: i32, top: i32) {
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
}

fn heard(control: isize, code: u32) -> bool {
    NOTIFICATIONS
        .lock()
        .map(|log| log.contains(&(control, code)))
        .unwrap_or(false)
}

fn text_of(handle: isize) -> String {
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

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("deadline")
}

fn write(handle: isize, value: &str) -> Vec<agent_desktop_core::ActionStep> {
    let element = root_from_hwnd(handle, deadline()).expect("the control resolves");
    set_value_steps(&element, value, InteractionPolicy::headless(), deadline())
        .expect("set-value delivers")
}

#[test]
fn set_value_on_a_combo_box_edit_tells_the_combo_box_owner() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let steps = write(owner.combo_edit(), r"C:\out\written-name.txt");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(steps[0].verified(), Some(true));
    assert_eq!(text_of(owner.combo_edit()), r"C:\out\written-name.txt");
    assert!(
        heard(combo, CBN_EDITCHANGE),
        "the owner heard CBN_EDITCHANGE"
    );
}

#[test]
fn set_value_on_the_combo_box_itself_writes_its_edit() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let steps = write(combo, "through the combo box");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(owner.combo_edit()), "through the combo box");
    assert!(heard(combo, CBN_EDITCHANGE));
}

#[test]
fn set_value_on_a_multiline_edit_sends_en_change() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(MULTILINE_ID);
    let steps = write(edit, "first line");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(edit), "first line");
    assert!(heard(edit, EN_CHANGE), "the owner heard EN_CHANGE");
}

#[test]
fn set_value_on_a_single_line_edit_sends_en_change() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(SINGLE_LINE_ID);
    let steps = write(edit, "single line");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(steps[0].verified(), Some(true));
    assert!(heard(edit, EN_CHANGE));
}

#[test]
fn a_value_pattern_write_to_a_combo_box_edit_tells_the_owner_nothing() {
    use uiautomation::patterns::UIValuePattern;
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let element = root_from_hwnd(owner.combo_edit(), deadline()).expect("the edit resolves");
    element
        .0
        .get_pattern::<UIValuePattern>()
        .expect("the edit advertises ValuePattern")
        .set_value("silent write")
        .expect("ValuePattern.SetValue succeeds");
    assert_eq!(text_of(owner.combo_edit()), "silent write");
    assert!(
        !heard(combo, CBN_EDITCHANGE),
        "WM_SETTEXT on a combo box edit sends no change notification"
    );
}

#[test]
fn a_value_pattern_write_to_a_multiline_edit_tells_the_owner_nothing() {
    use uiautomation::patterns::UIValuePattern;
    ensure_test_apartment();
    let owner = EditOwner::create();
    let edit = owner.control(MULTILINE_ID);
    let element = root_from_hwnd(edit, deadline()).expect("the edit resolves");
    element
        .0
        .get_pattern::<UIValuePattern>()
        .expect("the edit advertises ValuePattern")
        .set_value("silent write")
        .expect("ValuePattern.SetValue succeeds");
    assert_eq!(text_of(edit), "silent write");
    assert!(
        !heard(edit, EN_CHANGE),
        "WM_SETTEXT on a multiline edit sends no change notification"
    );
}

#[test]
fn clear_on_a_combo_box_edit_tells_the_combo_box_owner() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    let combo = owner.control(COMBO_ID);
    let element = root_from_hwnd(owner.combo_edit(), deadline()).expect("the edit resolves");
    let steps =
        clear_steps(&element, InteractionPolicy::headless(), deadline()).expect("clear delivers");
    assert_eq!(steps[0].label(), EDIT_LABEL);
    assert_eq!(text_of(owner.combo_edit()), "");
    assert!(heard(combo, CBN_EDITCHANGE));
}

#[test]
fn read_only_edits_and_drop_down_lists_keep_their_value_pattern() {
    ensure_test_apartment();
    let owner = EditOwner::create();
    for id in [READ_ONLY_ID, DROP_LIST_ID] {
        let element = root_from_hwnd(owner.control(id), deadline()).expect("resolves");
        assert_eq!(
            edit_target(&element, "value", deadline()),
            EditTarget::NotEdit,
            "control {id}"
        );
    }
    let element = root_from_hwnd(owner.control(MULTILINE_ID), deadline()).expect("resolves");
    assert_eq!(
        edit_target(&element, "with\0nul", deadline()),
        EditTarget::NotEdit,
        "a value holding U+0000 keeps ValuePattern"
    );
    let element = root_from_hwnd(owner.control(SINGLE_LINE_ID), deadline()).expect("resolves");
    assert_eq!(
        edit_target(&element, "value", deadline()),
        EditTarget::Edit(owner.control(SINGLE_LINE_ID))
    );
}
