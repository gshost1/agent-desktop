use crate::{
    AdapterError, AppError, ErrorCode, OpenUrlRequest, ProcessIdentity, SnapshotSurface,
    WindowInfo, adapter::PlatformAdapter,
};
use serde_json::Value;

/// How long `open-url` waits for the page's load event by default.
pub const DEFAULT_OPEN_URL_TIMEOUT_MS: u64 = 15_000;

pub struct OpenUrlArgs {
    pub url: String,
    pub app: String,
    pub new_tab: bool,
    pub window_id: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// Loads `url` in the browser named by `--app`: in the tab its window shows,
/// or in a new background tab with `--new-tab`. The adapter does the loading
/// and reports which channel did it; nothing is focused or raised. It holds
/// the interaction lease while it loads, as every command that changes an
/// application does, so it never navigates a page another session is driving.
pub fn execute(args: OpenUrlArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    validate_url(&args.url)?;
    let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_OPEN_URL_TIMEOUT_MS);
    crate::wait_timeout_duration(timeout_ms)?;
    let deadline = crate::Deadline::after(timeout_ms)?;
    let window = target_window(adapter, &args, deadline)?;
    let process = match &window {
        Some(window) => window_process(window)?,
        None => {
            let app = crate::system::app_lookup::resolve_app(Some(&args.app), adapter, deadline)?;
            crate::system::app_lookup::process_identity(&app)?
        }
    };
    let lease = adapter
        .acquire_interaction_lease(deadline)
        .map_err(AdapterError::unsent)?;
    let opened = adapter.open_url(&OpenUrlRequest {
        process,
        window: window.as_ref(),
        url: &args.url,
        new_tab: args.new_tab,
        deadline: lease.deadline(),
    })?;
    Ok(serde_json::to_value(opened)?)
}

/// The window whose visible tab is replaced. A new tab needs no window, so a
/// browser with none open can still take one.
fn target_window(
    adapter: &dyn PlatformAdapter,
    args: &OpenUrlArgs,
    deadline: crate::Deadline,
) -> Result<Option<WindowInfo>, AppError> {
    let resolved = crate::snapshot::resolve_window_for_surface(
        adapter,
        Some(&args.app),
        args.window_id.as_deref(),
        SnapshotSurface::Web,
        deadline,
    );
    match resolved {
        Ok(window) => Ok(Some(window)),
        Err(_) if args.new_tab && args.window_id.is_none() => Ok(None),
        Err(error) => Err(error),
    }
}

fn window_process(window: &WindowInfo) -> Result<ProcessIdentity, AppError> {
    let instance = window
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::ActionNotSupported,
                "The browser window has no process-instance identity",
            )
            .unsent()
        })?;
    Ok(ProcessIdentity::new(window.pid, instance))
}

/// A URL must name its scheme. `javascript:` runs script in the page rather
/// than loading one, so it is refused.
fn validate_url(url: &str) -> Result<(), AppError> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .filter(|scheme| {
            !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
        });
    match scheme {
        Some(scheme) if scheme.eq_ignore_ascii_case("javascript") => {
            Err(AppError::invalid_input_with_suggestion(
                "open-url does not run javascript: URLs",
                "Pass a page URL such as https://example.com",
            ))
        }
        Some(_) if !url.trim().is_empty() && !url.contains(char::is_whitespace) => Ok(()),
        _ => Err(AppError::invalid_input_with_suggestion(
            format!("'{url}' is not an absolute URL"),
            "Include the scheme, for example https://example.com",
        )),
    }
}

#[cfg(test)]
#[path = "open_url_tests.rs"]
mod tests;
