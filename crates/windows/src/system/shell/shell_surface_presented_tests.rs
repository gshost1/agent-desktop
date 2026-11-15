//! Windows-only: the refusals behind a shell-surface window id. A surface is
//! presented only under the exact handle its row resolves to and only while
//! the chain's top-level window is shown, and a presented immersive surface
//! counts as focused only for the process that owns it. In-process windows
//! stand in for the shell's class chain, so these cases need no shell state.

use agent_desktop_core::{ErrorCode, ProcessId, SnapshotSurface, WindowInfo, WindowState};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow,
    WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

use super::{
    SurfaceDismiss, SurfaceFamily, SurfaceKindRow, SurfaceRaise, presented_surface_at,
    presented_surface_in, presents_immersive_for, row_for,
};
use crate::system::fixture::live_identity::window_info_for_current_process;
use crate::system::fixture::test_time::deadline;
use crate::system::window::window_resolve::resolve_window_strict;
use crate::tree::fixture::fixture_window::{register_class_with_proc, unique_class_name, wide};

const TOOLBAR_CLASS: &str = "ToolbarWindow32";

unsafe extern "system" fn plain_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// A window this test created, destroyed on drop.
struct OwnedWindow(isize);

impl Drop for OwnedWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0 as _);
        }
    }
}

fn ensure_toolbar_class() {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| {
        let _ = register_class_with_proc(TOOLBAR_CLASS, plain_proc);
    });
}

fn create(class: &str, parent: Option<&OwnedWindow>) -> OwnedWindow {
    let name = wide(class);
    let (style, ex_style) = match parent {
        Some(_) => (WS_CHILD | WS_VISIBLE, 0),
        None => (WS_POPUP, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE),
    };
    let handle = unsafe {
        CreateWindowExW(
            ex_style,
            name.as_ptr(),
            std::ptr::null(),
            style,
            0,
            0,
            40,
            20,
            parent.map_or(std::ptr::null_mut(), |parent| parent.0 as _),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    assert!(!handle.is_null(), "the {class} window is created");
    OwnedWindow(handle as isize)
}

/// A shown top-level window of a class unique to this call, with a
/// `ToolbarWindow32` child: the shape of the tray-overflow chain.
fn staged_chain() -> (String, OwnedWindow, OwnedWindow) {
    ensure_toolbar_class();
    let top_class = unique_class_name();
    register_class_with_proc(&top_class, plain_proc).expect("the top class registers");
    let top = create(&top_class, None);
    unsafe {
        let _ = ShowWindow(top.0 as _, SW_SHOWNOACTIVATE);
    }
    let bar = create(TOOLBAR_CLASS, Some(&top));
    (top_class, top, bar)
}

fn rows_for_chain(top_class: &str) -> &'static [SurfaceKindRow] {
    let top: &'static str = Box::leak(top_class.to_owned().into_boxed_str());
    let chain: &'static [&'static str] = Box::leak(vec![top, TOOLBAR_CLASS].into_boxed_slice());
    Box::leak(
        vec![SurfaceKindRow {
            kind: SnapshotSurface::SystemTrayOverflow,
            family: SurfaceFamily::Win32Class(chain),
            raise: SurfaceRaise::AlreadyRaised,
            dismiss: SurfaceDismiss::None,
            exists_on_build: true,
            capability_holder: None,
        }]
        .into_boxed_slice(),
    )
}

fn presented_id(rows: &'static [SurfaceKindRow], window: &OwnedWindow) -> Option<String> {
    presented_surface_in(rows, window.0 as _, deadline(5_000))
        .expect("the staged chain is readable")
        .map(|(info, _)| info.id)
}

#[test]
fn the_handle_a_shown_chain_resolves_to_is_presented() {
    let (top_class, _top, bar) = staged_chain();
    let rows = rows_for_chain(&top_class);

    assert_eq!(
        presented_id(rows, &bar),
        Some(format!("w-{}", bar.0 as usize))
    );
}

#[test]
fn a_same_class_window_the_chain_does_not_resolve_to_is_not_presented() {
    let (top_class, _top, _bar) = staged_chain();
    let rows = rows_for_chain(&top_class);
    let (_other_class, other_top, _other_bar) = staged_chain();
    let decoy = create(TOOLBAR_CLASS, Some(&other_top));

    assert_eq!(
        presented_id(rows, &decoy),
        None,
        "a ToolbarWindow32 the row did not resolve to is not the presented surface"
    );
}

#[test]
fn a_toolbar_whose_top_window_is_hidden_is_not_presented() {
    let (top_class, top, bar) = staged_chain();
    let rows = rows_for_chain(&top_class);
    unsafe {
        let _ = ShowWindow(top.0 as _, SW_HIDE);
    }

    assert_eq!(
        presented_id(rows, &bar),
        None,
        "a toolbar that stays materialized under a hidden top window is dismissed"
    );
}

#[test]
fn a_toolbar_child_of_an_application_window_is_refused_as_not_found() {
    crate::tree::fixture::bootstrap();
    let (_top_class, _top, bar) = staged_chain();
    let presented = presented_surface_at(bar.0 as _, deadline(10_000))
        .expect("the shell surfaces are readable");
    assert!(
        presented.is_none(),
        "an application's ToolbarWindow32 is not a shell surface: {:?}",
        presented.map(|(info, _)| info.id)
    );

    let error = resolve_window_strict(&window_info_for_current_process(bar.0), deadline(10_000))
        .expect_err("a child window outside the inventory that no surface presents is refused");

    assert_eq!(error.code, ErrorCode::WindowNotFound, "{error:?}");
}

fn immersive_info(pid: u32) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: String::new(),
        app: "startmenuexperiencehost.exe".into(),
        pid: ProcessId::from(pid),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    }
}

#[test]
fn a_presented_immersive_surface_counts_only_for_its_own_process() {
    let start = row_for(SnapshotSurface::StartMenu).expect("start has a row");
    let taskbar = row_for(SnapshotSurface::Taskbar).expect("the taskbar has a row");

    assert!(presents_immersive_for(
        Some((immersive_info(42), start)),
        ProcessId::from(42u32)
    ));
    assert!(
        !presents_immersive_for(Some((immersive_info(42), start)), ProcessId::from(7u32)),
        "another process's presented surface is not the caller's focus"
    );
    assert!(!presents_immersive_for(
        Some((immersive_info(42), taskbar)),
        ProcessId::from(42u32)
    ));
    assert!(!presents_immersive_for(None, ProcessId::from(42u32)));
}
