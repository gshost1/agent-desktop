use serde_json::json;

use crate::{AdapterError, ErrorCode, PlatformAdapter, ProcessIdentity, WindowInfo};

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
) -> Result<bool, AdapterError> {
    let Some(expected) = entry_focus_window(entry, adapter, lease)? else {
        return Ok(false);
    };
    focus_exact_window(&expected, adapter, context, lease).map(|_| true)
}

/// The window a headed focus of this ref raises, resolved the way
/// [`focus_entry_window`] resolves it, or `None` for a ref captured through a
/// process owner, which has no window to raise.
pub(crate) fn entry_focus_window(
    entry: &crate::RefEntry,
    adapter: &dyn PlatformAdapter,
    lease: &crate::InteractionLease,
) -> Result<Option<WindowInfo>, AdapterError> {
    entry
        .process
        .identity()
        .ok_or_else(|| headed_refusal(MISSING_PROCESS_IDENTITY))?;
    if entry.source.is_process_owned() {
        return Ok(None);
    }
    adapter
        .resolve_ref_focus_window(entry, lease.deadline())
        .map(Some)
        .map_err(AdapterError::unsent)
}

/// Whether two reads name the same window of the same process instance, the
/// identity a headed focus requires of the window it raises.
pub(crate) fn same_window(seen: &WindowInfo, expected: &WindowInfo) -> bool {
    seen.id == expected.id
        && seen.pid == expected.pid
        && seen.process_instance == expected.process_instance
}

/// Raises the application a ref came from, for input that needs the
/// application rather than one of its windows focused, and returns whether it
/// did, with the same refusals as [`focus_entry_window`].
pub(crate) fn focus_entry_app(
    entry: &crate::RefEntry,
    adapter: &dyn PlatformAdapter,
    lease: &crate::InteractionLease,
) -> Result<bool, AdapterError> {
    let process = entry
        .process
        .identity()
        .ok_or_else(|| headed_refusal(MISSING_PROCESS_IDENTITY))?;
    adapter
        .focus_app(process, lease)
        .map_err(AdapterError::after_focus)?;
    Ok(true)
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
) -> Result<WindowInfo, AdapterError> {
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
) -> Result<WindowInfo, AdapterError> {
    let live = adapter
        .resolve_window_strict(expected, lease.deadline())
        .map_err(AdapterError::unsent)?;
    if !same_window(&live, expected) {
        return Err(AdapterError::stale_ref_because(
            "Headed target window belongs to a different process instance",
        ));
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
