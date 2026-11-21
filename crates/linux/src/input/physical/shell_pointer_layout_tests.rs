use agent_desktop_core::{AdapterError, DisplayInfo, Rect};

use super::shell_layout;
use crate::input::backend::Backend;
use crate::input::mouse::pointer_plan::plan_move;
use crate::input::wayland::pointer_gesture::{PointerAct, translate};
use crate::system::DisplayLayout;

fn displays() -> Vec<DisplayInfo> {
    vec![DisplayInfo {
        id: "Meta-1".into(),
        bounds: Rect {
            x: 0.,
            y: 0.,
            width: 800.,
            height: 600.,
        },
        is_primary: true,
        scale: 2.,
    }]
}

#[test]
fn a_wayland_shell_click_keeps_logical_layout_scaling_through_stream_translation() {
    let layout = shell_layout(
        &Backend::Wayland,
        || panic!("Wayland must not use the unscaled display-list path"),
        || {
            Ok(DisplayLayout {
                displays: displays(),
                stream_scaled: true,
            })
        },
    )
    .unwrap();
    let point = crate::input::physical::shell_aim::aim(
        Rect {
            x: 90.,
            y: 140.,
            width: 20.,
            height: 20.,
        },
        &layout.displays,
    )
    .unwrap();
    let gesture = translate(&plan_move(point).unwrap().steps, Vec::new(), &layout).unwrap();
    let at = gesture
        .acts
        .iter()
        .find_map(|act| match act {
            PointerAct::Motion(at) => Some(at),
            _ => None,
        })
        .unwrap();
    assert_eq!((at.x, at.y), (200., 300.));
}

#[test]
fn x11_shell_layout_stays_unscaled_and_wayland_layout_errors_are_not_hidden() {
    let layout = shell_layout(
        &Backend::X11(":100".into()),
        || Ok(displays()),
        || panic!("X11 must not request a Wayland layout"),
    )
    .unwrap();
    assert!(!layout.stream_scaled);
    let error = shell_layout(
        &Backend::Wayland,
        || panic!("no fallback to unscaled coordinates"),
        || Err(AdapterError::not_supported("unknown layout")),
    )
    .unwrap_err();
    assert_eq!(
        error.code,
        agent_desktop_core::ErrorCode::PlatformNotSupported
    );
}
