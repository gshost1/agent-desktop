use std::time::Duration;

use crate::{
    AdapterError, AppError, DeliverySemantics, ErrorCode, adapter::PlatformAdapter,
    system::cdp_endpoint, system::launch_options::LaunchOptions,
    system::launch_result::LaunchResult, tree::renderer_kind::RendererKind,
};
use serde_json::Value;

/// The largest slice of the overall deadline set aside for verifying the
/// DevTools endpoint once the process is launched.
const MAX_PROBE_RESERVE: Duration = Duration::from_millis(5_000);

pub struct LaunchArgs {
    pub app: String,
    pub options: LaunchOptions,
    /// Where the launched browser saves downloads. Needs `--cdp`. Launch
    /// asks the adapter to write it into the browser profile; failing that,
    /// every later DevTools connection points downloads here.
    pub download_dir: Option<std::path::PathBuf>,
}

pub fn execute(args: LaunchArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let LaunchArgs {
        app,
        mut options,
        download_dir,
    } = args;
    crate::wait_timeout_duration(options.timeout_ms)?;
    let download_dir = download_dir
        .map(|dir| crate::system::download_dir::prepare_download_dir(&dir, options.cdp_port))
        .transpose()?;
    let deadline = if options.timeout_ms == 0 {
        crate::Deadline::standard()?
    } else {
        crate::Deadline::after(options.timeout_ms)?
    };
    let requested_cdp_port = options.cdp_port;
    if let Some(requested_port) = requested_cdp_port {
        reject_conflicting_cdp_switch(&options)?;
        reject_if_already_running(&app, adapter, deadline)?;
        let resolved_port = resolve_cdp_port(requested_port)?;
        options
            .args
            .push(format!("--remote-debugging-port={resolved_port}"));
        options
            .args
            .push("--remote-debugging-address=127.0.0.1".to_owned());
        options.cdp_port = Some(resolved_port);
    }
    let probe_budget = requested_cdp_port.map(|_| probe_reserve(deadline.remaining()));
    let launch_deadline = probe_budget.map_or(deadline, |reserve| {
        deadline.capped(deadline.remaining().saturating_sub(reserve))
    });
    let download_scope = download_dir
        .as_deref()
        .map(|dir| prepare_downloads(adapter, &options, dir));
    let lease = adapter.acquire_interaction_lease(launch_deadline)?;
    let mut launched = adapter.launch_app(&app, &options, &lease)?;
    if let Some(port) = options.cdp_port {
        let granted = deadline.remaining();
        let mut endpoint = verify_cdp_endpoint(port, deadline, granted, &launched, adapter)?;
        endpoint.download_dir = download_dir;
        endpoint.download_scope = download_scope;
        remember_endpoint(adapter, &launched, &endpoint);
        launched.cdp = Some(endpoint);
    }
    if launched.suggestion.is_none() {
        launched.suggestion = launch_suggestion(requested_cdp_port.is_some(), &launched);
    }
    Ok(serde_json::to_value(launched)?)
}

/// Asks the adapter to write the download folder into the browser profile
/// before launch. The launch can still point downloads there over each
/// DevTools connection, so a refusal is logged and the connection scope
/// stands.
fn prepare_downloads(
    adapter: &dyn PlatformAdapter,
    options: &LaunchOptions,
    download_dir: &str,
) -> crate::DownloadScope {
    adapter
        .prepare_cdp_downloads(options, download_dir)
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, "launch: the profile download folder was not set");
            crate::DownloadScope::Connection
        })
}

/// A verified endpoint is offered to the adapter so a later invocation can
/// reach the same browser. The launch already succeeded, so a failure to
/// record it is logged and the result stands.
fn remember_endpoint(
    adapter: &dyn PlatformAdapter,
    launched: &LaunchResult,
    endpoint: &cdp_endpoint::CdpEndpoint,
) {
    let Some(instance) = launched.process_instance.as_deref() else {
        return;
    };
    let process = crate::ProcessIdentity::new(launched.pid, instance);
    if let Err(error) = adapter.remember_cdp_endpoint(process, endpoint) {
        tracing::warn!(error = %error, "launch: DevTools endpoint was not recorded");
    }
}

/// The probe needs some of the overall deadline reserved for itself, or a
/// generous overall timeout would let the launch step spend it all and leave
/// nothing to verify the endpoint with. Reserving a quarter of the total,
/// capped at `MAX_PROBE_RESERVE`, keeps the split proportionate for short
/// deadlines and bounded for long ones.
fn probe_reserve(total: Duration) -> Duration {
    (total / 4).min(MAX_PROBE_RESERVE)
}

/// Guidance the calling agent reads at the moment of use, not prose it has
/// to infer: a Chromium app launched without `--cdp` is nudged toward the
/// flag that opens the web-content door, and a verified endpoint points at
/// the web surface that reads and drives the page through it.
fn launch_suggestion(cdp_requested: bool, launched: &LaunchResult) -> Option<String> {
    if launched.cdp.is_some() {
        return Some(format!(
            "Next: load a page with `open-url <url> --app \"{app}\"`, read it with \
             `snapshot --app \"{app}\" --surface web -i`, and act on its refs with the usual \
             commands; `data.channel` is `cdp` when the page was read over this endpoint. \
             Do not hand-roll raw CDP or call app-internal APIs. Native menus, dialogs, \
             windows, and screenshots stay on the accessibility surfaces.",
            app = launched.app
        ));
    }
    if !cdp_requested && launched.renderer == Some(RendererKind::Chromium) {
        return Some(
            "Chromium app: for web-content work, run close-app and then launch --cdp, then \
             read the page with snapshot --surface web. Accessibility commands still cover \
             everything, including native menus and dialogs."
                .to_owned(),
        );
    }
    None
}

/// Our own `--remote-debugging-port`, `--remote-debugging-address`, and
/// `--remote-allow-origins` switches are how `--cdp` pins the DevTools
/// surface to loopback and works at all; a caller who also passes any of
/// them by hand would silently race or widen it, so this is rejected before
/// launch rather than left to whichever switch Launch Services keeps.
fn reject_conflicting_cdp_switch(options: &LaunchOptions) -> Result<(), AppError> {
    let conflicts = options.args.iter().any(|arg| {
        arg.starts_with("--remote-debugging-port")
            || arg == "--remote-debugging-pipe"
            || arg.starts_with("--remote-debugging-address")
            || arg.starts_with("--remote-allow-origins")
    });
    if !conflicts {
        return Ok(());
    }
    Err(AppError::Adapter(
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch already specifies a DevTools remote-debugging switch",
        )
        .with_details(serde_json::json!({ "kind": "cdp_switch_conflict" }))
        .with_suggestion(
            "--cdp owns the remote-debugging switches (port, address, allow-origins) — pass \
             --cdp instead of setting them yourself.",
        )
        .with_disposition(DeliverySemantics::not_delivered()),
    ))
}

/// The DevTools port only exists on the process that was launched with it;
/// attaching to an instance already running without it would report success
/// while the endpoint stays absent, so this is checked before launch instead
/// of discovered only once the endpoint fails to answer. Only workspace-
/// registered applications count: `list_apps` also returns ps-derived helper
/// processes with no bundle ID and no presentation, and the launch path can
/// never attach to one of those even when its name happens to match.
fn reject_if_already_running(
    id: &str,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<(), AppError> {
    let running = adapter.list_apps(deadline)?;
    let stem = launch_path_stem(id);
    let pids: Vec<_> = running
        .iter()
        .filter(|app| app.presentation.is_some())
        .filter(|app| app.matches_identifier(id) || stem.is_some_and(|s| app.matches_identifier(s)))
        .map(|app| app.pid)
        .collect();
    if pids.is_empty() {
        return Ok(());
    }
    Err(AppError::Adapter(
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The application is already running without the DevTools port",
        )
        .with_details(serde_json::json!({ "kind": "cdp_requires_fresh_launch", "pids": pids }))
        .with_suggestion(
            "The DevTools port only exists for a process launched with it. Quit the app with \
             close-app, then launch again with --cdp — or drive the running instance with \
             accessibility commands (snapshot/click).",
        )
        .with_disposition(DeliverySemantics::not_delivered()),
    ))
}

fn launch_path_stem(id: &str) -> Option<&str> {
    if !id.contains(['/', '\\']) {
        return None;
    }
    std::path::Path::new(id).file_stem()?.to_str()
}

fn resolve_cdp_port(requested: u16) -> Result<u16, AppError> {
    if requested == 0 {
        return cdp_endpoint::pick_free_port();
    }
    if !cdp_endpoint::port_is_free(requested) {
        return Err(AppError::Adapter(
            AdapterError::new(
                ErrorCode::InvalidArgs,
                "Requested --cdp port is already in use",
            )
            .with_details(serde_json::json!({ "kind": "cdp_port_in_use", "port": requested }))
            .with_disposition(DeliverySemantics::not_delivered()),
        ));
    }
    Ok(requested)
}

/// Verifies the endpoint by observation instead of trusting the switch: the
/// app is left running either way, so a probe failure reports what exists
/// (pid, process instance, the budget the probe was given) layered onto the
/// probe's own message and evidence rather than a second, separately worded
/// error that could drift from what was actually observed.
/// `probe_budget` is the time the probe actually received: the deadline's
/// remainder at probe start, not the pre-launch reserve, which a fast launch
/// leaves far behind.
fn verify_cdp_endpoint(
    port: u16,
    deadline: crate::Deadline,
    probe_budget: Duration,
    launched: &LaunchResult,
    adapter: &dyn PlatformAdapter,
) -> Result<cdp_endpoint::CdpEndpoint, AppError> {
    let endpoint = cdp_endpoint::probe(port, deadline)
        .map_err(|error| augment_probe_error(error, launched, probe_budget))?;
    let Some(instance) = launched.process_instance.as_deref() else {
        return Err(augment_probe_error(
            cdp_endpoint::unattributed(port),
            launched,
            probe_budget,
        ));
    };
    let process = crate::ProcessIdentity::new(launched.pid, instance);
    match adapter.process_owns_tcp_listener(process, port, deadline) {
        Ok(true) => Ok(endpoint),
        Ok(false) => Err(augment_probe_error(
            cdp_endpoint::unattributed(port),
            launched,
            probe_budget,
        )),
        Err(error) => Err(augment_probe_error(
            AppError::Adapter(error.with_disposition(DeliverySemantics::delivered_unverified())),
            launched,
            probe_budget,
        )),
    }
}

fn augment_probe_error(
    error: AppError,
    launched: &LaunchResult,
    probe_budget: Duration,
) -> AppError {
    let AppError::Adapter(inner) = error else {
        return error;
    };
    let extra = serde_json::json!({
        "pid": launched.pid,
        "process_instance": launched.process_instance,
        "probe_budget_ms": probe_budget.as_millis(),
    });
    let merged = cdp_endpoint::merge_object_details(
        inner
            .details
            .clone()
            .unwrap_or_else(|| serde_json::json!({})),
        extra,
    );
    AppError::Adapter(inner.with_details(merged).with_suggestion(
        "The app is running but its DevTools endpoint could not be verified as belonging to the \
         launched process. Drive it with accessibility commands, or close-app it if it is unwanted.",
    ))
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "launch_cdp_test_support.rs"]
mod cdp_test_support;

#[cfg(test)]
#[path = "launch_cdp_tests.rs"]
mod cdp_tests;

#[cfg(test)]
#[path = "launch_cdp_switch_tests.rs"]
mod cdp_switch_tests;

#[cfg(test)]
#[path = "launch_download_tests.rs"]
mod download_tests;
