use agent_desktop_core::AdapterError;

use crate::system::window::window_resolve::{ax_window_id_with_deadline, prepare_for_read};
use crate::system::window::window_resolve_errors::window_not_found;
use crate::tree::{AXElement, element_for_pid};

pub(crate) fn ax_window_element_for_number(
    pid: i32,
    window_number: i64,
    fallback_title: Option<&str>,
    deadline: std::time::Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let windows = windows_for_pid(pid, deadline)?;
    let mut unavailable_bridge = None;
    for window in &windows {
        prepare_for_read(window, deadline)?;
        if crate::tree::resolve::resolve_ax_read::read_string(window, "AXRole", deadline)?
            .as_deref()
            != Some("AXWindow")
        {
            continue;
        }
        match ax_window_id_with_deadline(window, deadline) {
            Ok(Some(actual)) if actual == window_number => return Ok(Some(window.clone())),
            Ok(_) => {}
            Err(error) if crate::system::window::window_bridge::is_unavailable(&error) => {
                unavailable_bridge = Some(error);
                break;
            }
            Err(error) => return Err(error),
        }
    }
    let Some(title) = fallback_title else {
        return match unavailable_bridge {
            Some(error) => Err(error),
            None => Ok(None),
        };
    };
    let mut single_match = None;
    for window in windows {
        prepare_for_read(&window, deadline)?;
        if crate::tree::resolve::resolve_ax_read::read_string(&window, "AXRole", deadline)?
            .as_deref()
            != Some("AXWindow")
        {
            continue;
        }
        if crate::tree::resolve::resolve_ax_read::read_string(&window, "AXTitle", deadline)?
            .as_deref()
            != Some(title)
        {
            continue;
        }
        if single_match.is_some() {
            return Err(AdapterError::ambiguous_target(format!(
                "Multiple accessibility windows of window w-{window_number} share its title"
            ))
            .with_details(serde_json::json!({
                "kind": "verified_window_title_ambiguous",
                "window_id": format!("w-{window_number}"),
            })));
        }
        single_match = Some(window);
    }
    match single_match {
        Some(window) => Ok(Some(window)),
        None => match unavailable_bridge {
            Some(error) => Err(error),
            None => Ok(None),
        },
    }
}

pub(crate) fn window_element_by_title(
    pid: i32,
    requested_title: &str,
    deadline: std::time::Instant,
) -> Result<AXElement, AdapterError> {
    let windows = windows_for_pid(pid, deadline)?;
    let mut exact = Vec::new();
    let mut window_count = 0_usize;
    let mut only_window = None;
    for window in windows {
        if crate::tree::resolve::resolve_ax_read::read_string(&window, "AXRole", deadline)?
            .as_deref()
            != Some("AXWindow")
        {
            continue;
        }
        window_count += 1;
        only_window = Some(window.clone());
        let title =
            crate::tree::resolve::resolve_ax_read::read_string(&window, "AXTitle", deadline)?;
        if title.as_deref() == Some(requested_title) && !requested_title.is_empty() {
            exact.push(window);
        }
    }
    if exact.len() == 1 {
        return Ok(exact.remove(0));
    }
    if exact.len() > 1 {
        return Err(AdapterError::ambiguous_target(format!(
            "Multiple windows have the exact title '{requested_title}'"
        ))
        .with_details(serde_json::json!({
            "kind": "window_title_ambiguous",
            "title": requested_title,
            "candidate_count": exact.len(),
        })));
    }
    if requested_title.is_empty() && window_count == 1 {
        return only_window.ok_or_else(|| window_not_found(requested_title));
    }
    Err(window_not_found(requested_title))
}

fn windows_for_pid(pid: i32, deadline: std::time::Instant) -> Result<Vec<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    Ok(
        crate::tree::resolve::resolve_ax_read::read_array(&app, "AXWindows", deadline)?
            .unwrap_or_default(),
    )
}
