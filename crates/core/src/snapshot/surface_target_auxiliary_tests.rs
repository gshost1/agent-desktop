use super::{resolve_window, resolve_window_for_surface};
use crate::{
    AdapterError, AppError, Deadline, ErrorCode, SnapshotSurface, WindowInfo, adapter::WindowFilter,
};

struct PanelApp {
    windows: Vec<WindowInfo>,
    auxiliary: &'static [&'static str],
}

impl crate::ObservationOps for PanelApp {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self.windows.clone())
    }

    fn is_auxiliary_window(&self, window: &WindowInfo, _deadline: Deadline) -> bool {
        self.auxiliary.contains(&window.id.as_str())
    }
}
impl crate::ActionOps for PanelApp {}
impl crate::InputOps for PanelApp {}
impl crate::SystemOps for PanelApp {}

fn window(id: &str, title: &str) -> WindowInfo {
    let mut window = crate::WindowInfo::for_process(&crate::AppInfo {
        name: "Fonts App".into(),
        pid: crate::ProcessId::new(9),
        process_instance: Some("inst-9".into()),
        bundle_id: None,
        presentation: None,
    });
    window.id = id.into();
    window.title = title.into();
    window.state.visible = Some(true);
    window
}

fn app(auxiliary: &'static [&'static str]) -> PanelApp {
    PanelApp {
        windows: vec![window("w-2", ""), window("w-1", "All Fonts")],
        auxiliary,
    }
}

fn by_app(adapter: &PanelApp) -> [Result<WindowInfo, AppError>; 2] {
    let deadline = Deadline::after(2_000).unwrap();
    [
        resolve_window_for_surface(
            adapter,
            Some("Fonts App"),
            None,
            SnapshotSurface::Window,
            deadline,
        ),
        resolve_window(adapter, Some("Fonts App"), None, deadline),
    ]
}

#[test]
fn an_auxiliary_suggestion_list_does_not_make_the_app_window_ambiguous() {
    for chosen in by_app(&app(&["w-2"])) {
        assert_eq!(chosen.unwrap().id, "w-1");
    }
}

#[test]
fn windows_that_are_all_auxiliary_or_all_primary_stay_ambiguous() {
    for auxiliary in [&[][..], &["w-1", "w-2"][..]] {
        for chosen in by_app(&app(auxiliary)) {
            let Err(AppError::Adapter(error)) = chosen else {
                panic!("expected an ambiguous target, got {chosen:?}");
            };
            assert_eq!(error.code, ErrorCode::AmbiguousTarget);
        }
    }
}

#[test]
fn a_focused_auxiliary_window_is_still_chosen_over_an_unfocused_document() {
    let mut adapter = app(&["w-2"]);
    adapter.windows[0].state.is_focused = true;
    for chosen in by_app(&adapter) {
        assert_eq!(chosen.unwrap().id, "w-2");
    }
}
