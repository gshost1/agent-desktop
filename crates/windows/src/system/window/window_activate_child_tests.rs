//! Windows-only: a child window id - the shape a tray-toolbar ref carries -
//! focuses by raising its own process's top-level window, since a child can
//! never own the foreground itself.

use super::*;
use crate::system::fixture::live_identity::window_info_for_current_process;
use crate::system::window::window_ops::is_root_foreground_window;
use agent_desktop_core::Deadline;

fn lease() -> InteractionLease {
    InteractionLease::guarded(Deadline::after(10_000).expect("deadline"), ()).expect("lease")
}

fn raise_handle(handle: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    unsafe {
        let _ = SetForegroundWindow(handle as _);
    }
}

/// A plain `STATIC` child of `parent`, destroyed on drop.
struct ChildWindow(isize);

impl ChildWindow {
    fn create(parent: isize) -> Self {
        use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, WS_CHILD, WS_VISIBLE};
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let handle = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                40,
                20,
                parent as _,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!handle.is_null(), "the child window is created");
        Self(handle as isize)
    }
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;
        unsafe {
            let _ = DestroyWindow(self.0 as _);
        }
    }
}

#[test]
fn a_child_window_id_raises_its_own_top_level_window() {
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    crate::tree::fixture::ensure_test_apartment();
    let (left_a, top_a) = crate::tree::fixture::fixture_window::on_screen_origin();
    let (left_b, top_b) = crate::tree::fixture::fixture_window::on_screen_origin();
    let target =
        crate::tree::fixture::LocalFixture::create_at(left_a, top_a).expect("target fixture");
    let stealer =
        crate::tree::fixture::LocalFixture::create_at(left_b, top_b).expect("stealer fixture");
    let child = ChildWindow::create(target.handle());
    raise_handle(stealer.handle());

    let focused = focus_window(&window_info_for_current_process(child.0), &lease());

    if let Err(error) = &focused {
        raise_handle(stealer.handle());
        let control = focus_window(&window_info_for_current_process(target.handle()), &lease());
        if control.is_err() {
            eprintln!(
                "skip child focus: this desktop declined the foreground to the top-level window too"
            );
            return;
        }
        panic!("the child id failed while its own top-level window focused: {error:?}");
    }
    assert!(
        is_root_foreground_window(child.0 as _),
        "the child's top-level window owns the foreground"
    );
    assert!(is_foreground_window(target.handle() as _));
}

fn set_cloak(handle: isize, cloaked: bool) -> i32 {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute};
    let value = i32::from(cloaked);
    unsafe {
        DwmSetWindowAttribute(
            handle as _,
            DWMWA_CLOAK as u32,
            (&value as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        )
    }
}

fn foreground() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    (unsafe { GetForegroundWindow() }) as isize
}

/// A cloaked window is not shown, so raising it would hand the foreground to
/// a window nobody can see: a dismissed Start stays cloaked, and before this
/// refusal the raise made it the foreground and reported success.
#[test]
fn a_cloaked_window_is_refused_before_any_raise() {
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    crate::tree::fixture::ensure_test_apartment();
    let (left, top) = crate::tree::fixture::fixture_window::on_screen_origin();
    let target = crate::tree::fixture::LocalFixture::create_at(left, top).expect("target fixture");
    assert_eq!(
        set_cloak(target.handle(), true),
        0,
        "the DWM cloaks the window"
    );
    assert!(super::super::window_enum::is_cloaked(target.handle() as _));

    let focused = focus_window(&window_info_for_current_process(target.handle()), &lease());
    let after = foreground();
    let _ = set_cloak(target.handle(), false);

    let error = focused.expect_err("a cloaked window is not raised");
    assert_eq!(error.code, ErrorCode::WindowNotFound, "{error:?}");
    assert_ne!(
        after,
        target.handle(),
        "the foreground never moved onto the cloaked window"
    );
}
