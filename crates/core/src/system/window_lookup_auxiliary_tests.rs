use crate::{
    AdapterError, AppError, AppInfo, Deadline, ErrorCode, ImageBuffer, ImageFormat,
    InteractionLease, OperationReport, ProcessIdentity, WindowInfo, WindowOp,
    adapter::{ScreenshotTarget, WindowFilter},
    commands::{
        focus_window::{self, FocusWindowArgs},
        helpers::AppArgs,
        minimize,
        screenshot::{self, ScreenshotArgs},
    },
};
use std::sync::Mutex;

const APP: &str = "Fonts App";
const PID: u32 = 9;
const INSTANCE: &str = "inst-9";

struct PanelApp {
    windows: Vec<WindowInfo>,
    auxiliary: &'static [&'static str],
    acted_on: Mutex<Vec<String>>,
}

impl PanelApp {
    fn acted_on(&self) -> Vec<String> {
        self.acted_on.lock().unwrap().clone()
    }

    fn record(&self, window: &WindowInfo) {
        self.acted_on.lock().unwrap().push(window.id.clone());
    }
}

impl crate::ObservationOps for PanelApp {
    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![app_info()])
    }

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
impl crate::SystemOps for PanelApp {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &WindowInfo,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn focus_window(
        &self,
        window: &WindowInfo,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.record(window);
        Ok(())
    }

    fn window_op(
        &self,
        window: &WindowInfo,
        _op: WindowOp,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.record(window);
        Ok(crate::adapter::test_report())
    }

    fn screenshot(
        &self,
        target: ScreenshotTarget,
        _deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        if let ScreenshotTarget::ExactWindow(window) = &target {
            self.record(window);
        }
        Ok(ImageBuffer {
            data: vec![1],
            format: ImageFormat::Png,
            width: 1,
            height: 1,
            scale_factor: 1.0,
        })
    }
}

fn app_info() -> AppInfo {
    AppInfo {
        name: APP.into(),
        pid: crate::ProcessId::new(PID),
        process_instance: Some(INSTANCE.into()),
        bundle_id: None,
        presentation: None,
    }
}

fn window(id: &str, title: &str) -> WindowInfo {
    let mut window = WindowInfo::for_process(&app_info());
    window.id = id.into();
    window.title = title.into();
    window.state.visible = Some(true);
    window
}

fn app(auxiliary: &'static [&'static str]) -> PanelApp {
    PanelApp {
        windows: vec![window("w-2", ""), window("w-1", "All Fonts")],
        auxiliary,
        acted_on: Mutex::new(Vec::new()),
    }
}

/// Runs every `--app` command that picks one window of the app and acts on it,
/// and returns each command's outcome.
fn every_app_command(adapter: &PanelApp) -> Vec<(&'static str, Result<(), AppError>)> {
    let by_app = || Some(APP.to_string());
    let headed_press = || {
        let lease = InteractionLease::guarded(Deadline::after(2_000)?, ())?;
        crate::actions::headed_focus::focus_process_window(
            ProcessIdentity {
                pid: crate::ProcessId::new(PID),
                instance: INSTANCE.into(),
            },
            adapter,
            &crate::CommandContext::default(),
            &lease,
        )
        .map(drop)
        .map_err(AppError::Adapter)
    };
    vec![
        (
            "focus-window",
            focus_window::execute(
                FocusWindowArgs {
                    window_id: None,
                    app: by_app(),
                    title: None,
                },
                adapter,
            )
            .map(drop),
        ),
        (
            "screenshot",
            screenshot::execute(
                ScreenshotArgs {
                    app: by_app(),
                    window_id: None,
                    screen: None,
                    output_path: None,
                },
                adapter,
            )
            .map(drop),
        ),
        (
            "minimize",
            minimize::execute(
                AppArgs {
                    app: by_app(),
                    window_id: None,
                },
                adapter,
            )
            .map(drop),
        ),
        ("press --headed", headed_press()),
    ]
}

#[test]
fn an_auxiliary_panel_does_not_make_an_app_command_ambiguous() {
    let adapter = app(&["w-2"]);
    for (command, outcome) in every_app_command(&adapter) {
        assert!(outcome.is_ok(), "{command}: {outcome:?}");
    }
    assert_eq!(adapter.acted_on(), vec!["w-1"; 4]);
}

#[test]
fn app_commands_stay_ambiguous_when_every_window_is_auxiliary_or_none_is() {
    for auxiliary in [&[][..], &["w-1", "w-2"][..]] {
        let adapter = app(auxiliary);
        for (command, outcome) in every_app_command(&adapter) {
            let Err(AppError::Adapter(error)) = outcome else {
                panic!("{command}: expected an ambiguous target, got {outcome:?}");
            };
            assert_eq!(error.code, ErrorCode::AmbiguousTarget, "{command}");
        }
        assert!(adapter.acted_on().is_empty());
    }
}

#[test]
fn two_standard_windows_beside_a_panel_stay_ambiguous_for_app_commands() {
    let mut adapter = app(&["w-2"]);
    adapter.windows.push(window("w-3", "Untitled"));
    for (command, outcome) in every_app_command(&adapter) {
        let Err(AppError::Adapter(error)) = outcome else {
            panic!("{command}: expected an ambiguous target, got {outcome:?}");
        };
        assert_eq!(error.code, ErrorCode::AmbiguousTarget, "{command}");
        assert_eq!(error.details.unwrap()["candidate_count"], 2, "{command}");
    }
    assert!(adapter.acted_on().is_empty());
}

#[test]
fn a_focused_panel_is_still_chosen_by_app_commands() {
    let mut adapter = app(&["w-2"]);
    adapter.windows[0].state.is_focused = true;
    for (command, outcome) in every_app_command(&adapter) {
        assert!(outcome.is_ok(), "{command}: {outcome:?}");
    }
    assert_eq!(adapter.acted_on(), vec!["w-2"; 4]);
}

#[test]
fn an_explicit_window_id_reaches_a_panel_beside_a_standard_window() {
    let adapter = app(&["w-2"]);
    minimize::execute(
        AppArgs {
            app: Some(APP.into()),
            window_id: Some("w-2".into()),
        },
        &adapter,
    )
    .expect("minimize by window id");
    assert_eq!(adapter.acted_on(), vec!["w-2"]);
}
