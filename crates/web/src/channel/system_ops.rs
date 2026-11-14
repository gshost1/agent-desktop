use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::system::launch_options::LaunchOptions;
use agent_desktop_core::system::launch_result::LaunchResult;
use agent_desktop_core::system::process_state::ProcessState;
use agent_desktop_core::{
    ActionResult, AdapterError, AdapterSession, AppInfo, CursorOverlayControl, Deadline,
    DismissAllNotificationsRequest, DismissNotificationRequest, DisplayInfo, DownloadInfo,
    DownloadScope, ImageBuffer, InteractionLease, InteractionPolicy, KeyCombo,
    NotificationActionRequest, NotificationFilter, NotificationInfo, OpenUrlRequest, OpenedPage,
    OperationReport, PermissionReport, PlatformAdapter, ProcessIdentity, RefEntry,
    ScreenshotTarget, SessionAffinity, SignalBaseline, SignalFilter, SnapshotSurface, SystemOps,
    WaitWakeup, WindowInfo, WindowOp,
};

use super::candidate_origin::CandidateOrigin;
use super::download::download_profile::save_in_profile;
use super::ref_routing::platform_entry;
use super::web_channel_adapter::WebChannelAdapter;

const REMEMBER_TIMEOUT_MS: u64 = 2_000;

impl<P: PlatformAdapter> SystemOps for WebChannelAdapter<P> {
    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        self.inner.update_cursor_overlay(control)
    }

    fn run_cursor_overlay_child(&self) -> Option<Result<(), AdapterError>> {
        self.inner.run_cursor_overlay_child()
    }

    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        self.inner.acquire_interaction_lease(deadline)
    }

    fn permission_report(&self, deadline: Deadline) -> Result<PermissionReport, AdapterError> {
        self.inner.permission_report(deadline)
    }

    fn preflight_permission_report(
        &self,
        deadline: Deadline,
    ) -> Result<PermissionReport, AdapterError> {
        self.inner.preflight_permission_report(deadline)
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        self.inner.unknown_accessibility_means_unsupported()
    }

    fn activate_renderer_accessibility(
        &self,
        process: ProcessIdentity,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.inner.activate_renderer_accessibility(process, lease)
    }

    fn request_permissions(
        &self,
        lease: &InteractionLease,
    ) -> Result<PermissionReport, AdapterError> {
        self.inner.request_permissions(lease)
    }

    fn start_permission_guide(&self) -> Result<&'static str, AdapterError> {
        self.inner.start_permission_guide()
    }

    fn focus_window(&self, win: &WindowInfo, lease: &InteractionLease) -> Result<(), AdapterError> {
        self.inner.focus_window(win, lease)
    }

    fn resolve_ref_focus_window(
        &self,
        entry: &RefEntry,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.inner
            .resolve_ref_focus_window(&platform_entry(entry), deadline)
    }

    fn focus_app(
        &self,
        process: ProcessIdentity,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.inner.focus_app(process, lease)
    }

    fn launch_app(
        &self,
        id: &str,
        options: &LaunchOptions,
        lease: &InteractionLease,
    ) -> Result<LaunchResult, AdapterError> {
        self.inner.launch_app(id, options, lease)
    }

    fn process_owns_tcp_listener(
        &self,
        process: ProcessIdentity,
        port: u16,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        self.inner
            .process_owns_tcp_listener(process, port, deadline)
    }

    fn remember_cdp_endpoint(
        &self,
        process: ProcessIdentity,
        endpoint: &CdpEndpoint,
    ) -> Result<(), AdapterError> {
        self.inner
            .remember_cdp_endpoint(process.clone(), endpoint)?;
        let deadline = Deadline::after(REMEMBER_TIMEOUT_MS)?;
        self.connector.remember(&process, endpoint, deadline)?;
        if endpoint.download_scope == Some(DownloadScope::Profile)
            && let Some(dir) = endpoint.download_dir.as_deref()
        {
            self.claim_existing_downloads(&process, std::path::Path::new(dir));
        }
        Ok(())
    }

    fn has_recorded_cdp_endpoint(&self, process: ProcessIdentity) -> bool {
        self.connector
            .candidates(&process)
            .iter()
            .any(|candidate| matches!(candidate.origin, CandidateOrigin::Registry { .. }))
    }

    /// Writes the download folder into the browser profile the launch
    /// arguments name, when it is one this may change.
    fn prepare_cdp_downloads(
        &self,
        options: &LaunchOptions,
        download_dir: &str,
    ) -> Result<DownloadScope, AdapterError> {
        save_in_profile(&options.args, std::path::Path::new(download_dir))
    }

    /// Loads the URL over CDP when an endpoint reaches the browser, and asks
    /// the platform adapter otherwise.
    fn open_url(&self, request: &OpenUrlRequest<'_>) -> Result<OpenedPage, AdapterError> {
        match self.open_web_url(request)? {
            Some(page) => Ok(page),
            None => self.inner.open_url(request),
        }
    }

    /// Reads the browser's downloads over CDP. A process no endpoint
    /// reaches is asked of the platform adapter, and keeps the web channel's
    /// refusal when the platform has no download support either.
    fn wait_for_download(
        &self,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<DownloadInfo, AdapterError> {
        match self.wait_web_download(&process, deadline) {
            Err(error) if is_endpoint_missing(&error) => self
                .inner
                .wait_for_download(process, deadline)
                .map_err(|platform| {
                    if is_unsupported(&platform) {
                        error
                    } else {
                        platform
                    }
                }),
            outcome => outcome,
        }
    }

    fn process_state(
        &self,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<ProcessState, AdapterError> {
        self.inner.process_state(process, deadline)
    }

    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        let mut surfaces = self.inner.supported_surfaces();
        if !surfaces.contains(&SnapshotSurface::Web) {
            surfaces.push(SnapshotSurface::Web);
        }
        surfaces
    }

    fn check_surface_support(&self, requested: SnapshotSurface) -> Result<(), AdapterError> {
        if requested == SnapshotSurface::Web {
            return Ok(());
        }
        self.inner.check_surface_support(requested)
    }

    fn subscribe_wait(
        &self,
        filter: &SignalFilter,
        deadline: Deadline,
    ) -> Result<Option<Box<dyn WaitWakeup>>, AdapterError> {
        self.inner.subscribe_wait(filter, deadline)
    }

    fn open_system_surface(
        &self,
        surface: SnapshotSurface,
        policy: InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<WindowInfo, AdapterError> {
        self.inner.open_system_surface(surface, policy, lease)
    }

    fn open_session(
        &self,
        affinity: &SessionAffinity,
        deadline: Deadline,
    ) -> Result<Box<dyn AdapterSession>, AdapterError> {
        self.inner.open_session(affinity, deadline)
    }

    fn capture_signal_baseline(
        &self,
        filter: &SignalFilter,
        deadline: Deadline,
    ) -> Result<SignalBaseline, AdapterError> {
        self.inner.capture_signal_baseline(filter, deadline)
    }

    fn close_app(
        &self,
        app: &AppInfo,
        force: bool,
        lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.inner.close_app(app, force, lease)
    }

    fn is_protected_process(&self, identifier: &str) -> bool {
        self.inner.is_protected_process(identifier)
    }

    fn is_blocked_combo(&self, combo: &KeyCombo) -> bool {
        self.inner.is_blocked_combo(combo)
    }

    fn screenshot(
        &self,
        target: ScreenshotTarget,
        deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        self.inner.screenshot(target, deadline)
    }

    fn screenshot_window_frame(
        &self,
        window: &WindowInfo,
        deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        self.inner.screenshot_window_frame(window, deadline)
    }

    fn list_displays(&self, deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        self.inner.list_displays(deadline)
    }

    fn focused_window(&self, deadline: Deadline) -> Result<Option<WindowInfo>, AdapterError> {
        self.inner.focused_window(deadline)
    }

    fn press_key_for_app(
        &self,
        process: ProcessIdentity,
        combo: &KeyCombo,
        policy: InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.inner.press_key_for_app(process, combo, policy, lease)
    }

    fn press_key_for_window(
        &self,
        window: &WindowInfo,
        combo: &KeyCombo,
        policy: InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.inner
            .press_key_for_window(window, combo, policy, lease)
    }

    fn wait_for_menu(
        &self,
        process: ProcessIdentity,
        open: bool,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.inner.wait_for_menu(process, open, deadline)
    }

    fn window_op(
        &self,
        win: &WindowInfo,
        op: WindowOp,
        lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.inner.window_op(win, op, lease)
    }

    fn resolve_window_strict(
        &self,
        win: &WindowInfo,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.inner.resolve_window_strict(win, deadline)
    }

    fn check_notification_support(&self, deadline: Deadline) -> Result<(), AdapterError> {
        self.inner.check_notification_support(deadline)
    }

    fn list_notifications(
        &self,
        filter: &NotificationFilter,
        policy: InteractionPolicy,
        deadline: Deadline,
        lease: Option<&InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        self.inner
            .list_notifications(filter, policy, deadline, lease)
    }

    fn dismiss_notification(
        &self,
        request: DismissNotificationRequest<'_>,
        lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        self.inner.dismiss_notification(request, lease)
    }

    fn dismiss_all_notifications(
        &self,
        request: DismissAllNotificationsRequest<'_>,
        lease: &InteractionLease,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        self.inner.dismiss_all_notifications(request, lease)
    }

    fn notification_action(
        &self,
        request: NotificationActionRequest<'_>,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.inner.notification_action(request, lease)
    }
}

fn is_endpoint_missing(error: &AdapterError) -> bool {
    crate::cdp::protocol_error::error_kind(error) == Some("cdp_endpoint_missing")
}

fn is_unsupported(error: &AdapterError) -> bool {
    matches!(
        error.code,
        agent_desktop_core::ErrorCode::PlatformNotSupported
            | agent_desktop_core::ErrorCode::ActionNotSupported
    )
}

#[cfg(test)]
#[path = "system_ops_tests.rs"]
mod tests;
