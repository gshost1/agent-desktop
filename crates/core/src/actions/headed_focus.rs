use serde_json::json;

use crate::{AdapterError, AppError, ErrorCode, PlatformAdapter, ProcessIdentity, WindowInfo};

/// Focuses the window a ref came from and returns whether it did. A ref
/// captured through a process owner has no window to raise, so it returns
/// `false`; any other ref without a window id is refused. Every refusal is
/// `not_delivered`, because focus always comes before the input it serves,
/// and one raised after the focus call also carries `preparatory_focus`.
pub(crate) fn focus_entry_window(
    entry: &crate::RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &crate::CommandContext,
    lease: &crate::InteractionLease,
) -> Result<bool, AppError> {
    entry
        .process
        .identity()
        .ok_or_else(|| headed_refusal(MISSING_PROCESS_IDENTITY))?;
    if entry.source.is_process_owned() {
        return Ok(false);
    }
    let expected = adapter
        .resolve_ref_focus_window(entry, lease.deadline())
        .map_err(AdapterError::unsent)?;
    focus_exact_window(&expected, adapter, context, lease).map(|_| true)
}

/// Raises the application a ref came from, for input that needs the
/// application rather than one of its windows focused.
pub(crate) fn focus_entry_app(
    entry: &crate::RefEntry,
    adapter: &dyn PlatformAdapter,
    lease: &crate::InteractionLease,
) -> Result<(), AdapterError> {
    let process = entry
        .process
        .identity()
        .ok_or_else(|| AdapterError::stale_ref_because("Target process identity is unavailable"))?;
    adapter
        .focus_app(process, lease)
        .map_err(AdapterError::after_focus)
}

pub fn saved_ref_focus_window(entry: &crate::RefEntry) -> Result<WindowInfo, AdapterError> {
    let process = entry
        .process
        .identity()
        .ok_or_else(|| headed_refusal(MISSING_PROCESS_IDENTITY))?;
    let window_id = required_text(
        entry.source.source_window_id.as_deref(),
        "Headed input requires an exact source window id",
    )?;
    Ok(WindowInfo {
        id: window_id.to_string(),
        title: entry.source.source_window_title.clone().unwrap_or_default(),
        app: entry.source.source_app.clone().unwrap_or_default(),
        pid: process.pid,
        process_instance: Some(process.instance),
        bounds: None,
        state: crate::WindowState::default(),
    })
}

pub(crate) fn focus_process_window(
    process: ProcessIdentity,
    adapter: &dyn PlatformAdapter,
    context: &crate::CommandContext,
    lease: &crate::InteractionLease,
) -> Result<WindowInfo, AppError> {
    let expected =
        crate::system::window_lookup::find_window_for_process(process, adapter, lease.deadline())
            .map_err(|error| error.into_adapter_error().unsent())?;
    focus_exact_window(&expected, adapter, context, lease)
}

pub(crate) fn focus_exact_window(
    expected: &WindowInfo,
    adapter: &dyn PlatformAdapter,
    context: &crate::CommandContext,
    lease: &crate::InteractionLease,
) -> Result<WindowInfo, AppError> {
    let live = adapter
        .resolve_window_strict(expected, lease.deadline())
        .map_err(AdapterError::unsent)?;
    if live.id != expected.id
        || live.pid != expected.pid
        || live.process_instance != expected.process_instance
    {
        return Err(AdapterError::stale_ref_because(
            "Headed target window belongs to a different process instance",
        )
        .into());
    }
    adapter
        .focus_window(&live, lease)
        .map_err(AdapterError::after_focus)?;
    context
        .trace_lazy(
            "input.focus_window",
            || json!({ "pid": live.pid, "window_id": live.id, "ok": true }),
        )
        .map_err(|error| error.into_adapter_error().after_focus())?;
    Ok(live)
}

fn required_text<'a>(value: Option<&'a str>, message: &str) -> Result<&'a str, AdapterError> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| headed_refusal(message))
}

const MISSING_PROCESS_IDENTITY: &str = "Headed input requires target process-instance identity";

/// A headed target that cannot be identified is refused before any input
/// starts, so the refusal says `not_delivered` as well as
/// `physical_delivery_started: false`.
fn headed_refusal(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::ActionNotSupported, message)
        .with_details(json!({ "physical_delivery_started": false }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "headed_focus_tests.rs"]
mod tests;
