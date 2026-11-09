use agent_desktop_core::{AdapterError, AppInfo, ErrorCode, system::launch_options::LaunchOptions};

pub(super) enum LaunchTarget {
    Existing { pid: i32, process_instance: String },
    New { baseline_instances: Vec<String> },
}

#[cfg(target_os = "macos")]
pub(super) fn launch_target(
    options: &LaunchOptions,
    mut initial: Vec<AppInfo>,
) -> Result<LaunchTarget, AdapterError> {
    if options.attach_if_running {
        if initial.len() > 1 {
            return Err(ambiguous_apps(&initial));
        }
        if let Some(app) = initial.pop() {
            let process_instance = required_instance(&app)?;
            return Ok(LaunchTarget::Existing {
                pid: super::process::process_identity::to_pid_t(app.pid)?,
                process_instance,
            });
        }
    }
    let baseline_instances = initial
        .iter()
        .map(required_instance)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LaunchTarget::New { baseline_instances })
}

#[cfg(target_os = "macos")]
pub(super) fn validate_launched_target(
    target: &LaunchTarget,
    launched: &(i32, String),
) -> Result<(), AdapterError> {
    match target {
        LaunchTarget::Existing {
            pid,
            process_instance,
        } => {
            if launched.0 == *pid && launched.1 == *process_instance {
                Ok(())
            } else {
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "NSWorkspace returned a different application while attaching",
                )
                .with_details(serde_json::json!({
                    "expected_pid": pid,
                    "returned_pid": launched.0,
                    "complete": false,
                })))
            }
        }
        LaunchTarget::New { baseline_instances } => {
            if baseline_instances.contains(&launched.1) {
                Err(AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "NSWorkspace reused an existing application during an exact fresh launch",
                )
                .with_details(serde_json::json!({
                    "returned_pid": launched.0,
                    "complete": false,
                })))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn ambiguous_apps(apps: &[AppInfo]) -> AdapterError {
    AdapterError::ambiguous_process_target(
        "More than one application instance matches the launch target",
        &apps.iter().map(|app| app.pid).collect::<Vec<_>>(),
    )
}

#[cfg(target_os = "macos")]
pub(super) fn required_instance(app: &AppInfo) -> Result<String, AdapterError> {
    app.process_instance.clone().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Running application has no exact process instance token",
        )
        .with_details(serde_json::json!({ "pid": app.pid, "complete": false }))
    })
}
