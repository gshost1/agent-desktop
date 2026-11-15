use crate::{
    AppError,
    adapter::{PlatformAdapter, WindowFilter},
};
use serde_json::{Value, json};
pub struct FocusWindowArgs {
    pub window_id: Option<String>,
    pub app: Option<String>,
    pub title: Option<String>,
}

pub fn execute(args: FocusWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let deadline = crate::Deadline::standard()?;
    let filter = WindowFilter {
        focused_only: false,
        app: args.app.clone(),
    };
    let windows = adapter.list_windows(&filter, deadline)?;

    let not_found = || {
        crate::AdapterError::new(crate::ErrorCode::WindowNotFound, "No matching window found")
            .with_suggestion("Run 'list-windows' to see available windows and their IDs.")
    };
    let ambiguous = "More than one window matches the focus target";
    let candidates = if let Some(id) = &args.window_id {
        windows
            .into_iter()
            .filter(|window| &window.id == id)
            .collect()
    } else if let Some(title) = &args.title {
        windows
            .into_iter()
            .filter(|window| window.title.contains(title.as_str()))
            .collect()
    } else if args.app.is_some() {
        return focus(
            adapter,
            crate::system::window_lookup::select_app_window(
                adapter,
                windows,
                not_found(),
                ambiguous,
                deadline,
            )?,
            deadline,
        );
    } else {
        return Err(AppError::invalid_input(
            "Provide --window-id, --app, or --title to identify the window",
        ));
    };
    let window = crate::system::window_lookup::select_window(candidates, not_found(), ambiguous)?;
    focus(adapter, window, deadline)
}

fn focus(
    adapter: &dyn PlatformAdapter,
    window: crate::WindowInfo,
    deadline: crate::Deadline,
) -> Result<Value, AppError> {
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let observed = crate::system::window_focus::focus_and_observe(adapter, &window, &lease)?;
    let focus_observed = observed.is_some();
    let focused = observed.unwrap_or_else(|| crate::system::window_focus::confirmed_target(window));
    Ok(json!({ "focused": focused, "focus_observed": focus_observed }))
}

#[cfg(test)]
#[path = "focus_window_tests.rs"]
mod tests;
