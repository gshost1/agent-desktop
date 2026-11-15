use crate::{
    AdapterError, AdapterSession, AppInfo, CursorOverlayControl, Deadline,
    DismissAllNotificationsRequest, DismissAllOutcome, DismissNotificationRequest, ImageBuffer,
    InteractionLease, InteractionPolicy, KeyCombo, NotificationActionRequest, NotificationFilter,
    NotificationInfo, NotificationOutcome, OperationReport, PermissionReport, PermissionState,
    ProcessIdentity, SessionAffinity, SignalBaseline, SignalFilter, WindowInfo, WindowOp,
    actions::action_result::ActionResult, system::display_info::DisplayInfo,
    system::screenshot_target::ScreenshotTarget,
};

pub trait SystemOps: Send + Sync {
    fn update_cursor_overlay(&self, _control: &CursorOverlayControl) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("update_cursor_overlay"))
    }

    fn run_cursor_overlay_child(&self) -> Option<Result<(), AdapterError>> {
        None
    }

    fn acquire_interaction_lease(
        &self,
        _deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        Err(AdapterError::not_supported("acquire_interaction_lease"))
    }

    /// An adapter that never overrides this has not probed permissions, which is not a denial:
    /// `Unknown` routes to `PLATFORM_NOT_SUPPORTED` rather than the misleading `PERM_DENIED`.
    fn permission_report(&self, _deadline: Deadline) -> Result<PermissionReport, AdapterError> {
        Ok(PermissionReport {
            accessibility: PermissionState::Unknown,
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::NotRequired,
        })
    }

    /// The report a command preflight reads, which never includes Automation.
    /// Probing Automation can wait on the OS permission daemon, so an adapter
    /// may leave it unprobed here.
    fn preflight_permission_report(
        &self,
        deadline: Deadline,
    ) -> Result<PermissionReport, AdapterError> {
        self.permission_report(deadline)
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        true
    }

    /// Performs one renderer-accessibility activation mutation and returns.
    /// Readiness polling belongs to core after the interaction lease is dropped.
    fn activate_renderer_accessibility(
        &self,
        _process: ProcessIdentity,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported(
            "activate_renderer_accessibility",
        ))
    }

    fn request_permissions(
        &self,
        lease: &InteractionLease,
    ) -> Result<PermissionReport, AdapterError> {
        self.permission_report(lease.deadline())
    }

    /// Starts a detached, on-screen guide that walks the user through granting
    /// the missing permissions, and returns without waiting for it. The result
    /// is `started`, `already_running`, or `not_needed`.
    fn start_permission_guide(&self) -> Result<&'static str, AdapterError> {
        Err(AdapterError::not_supported("start_permission_guide"))
    }

    /// Performs the platform-native focus/raise operation for the exact window.
    /// Core decides when focus is required; implementations must return only
    /// after that window is confirmed focused, or return an error.
    fn focus_window(
        &self,
        _win: &WindowInfo,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_window"))
    }

    fn resolve_ref_focus_window(
        &self,
        entry: &crate::RefEntry,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        crate::saved_ref_focus_window(entry)
    }

    fn focus_app(
        &self,
        _process: ProcessIdentity,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_app"))
    }

    fn launch_app(
        &self,
        _id: &str,
        _options: &crate::system::launch_options::LaunchOptions,
        _lease: &InteractionLease,
    ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
        Err(AdapterError::not_supported("launch_app"))
    }

    fn process_owns_tcp_listener(
        &self,
        _process: crate::ProcessIdentity,
        _port: u16,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Err(AdapterError::not_supported("process_owns_tcp_listener"))
    }

    /// Records a DevTools endpoint that launch verified the process owns, so
    /// a later invocation can find it. Adapters with no use for it accept it.
    fn remember_cdp_endpoint(
        &self,
        _process: ProcessIdentity,
        _endpoint: &crate::system::cdp_endpoint::CdpEndpoint,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    /// Whether `launch --cdp` recorded a DevTools endpoint for this exact
    /// process instance. Only the record is read; nothing is contacted.
    fn has_recorded_cdp_endpoint(&self, _process: ProcessIdentity) -> bool {
        false
    }

    /// Loads a URL in a browser's visible tab, or in a new background tab.
    fn open_url(
        &self,
        _request: &crate::OpenUrlRequest<'_>,
    ) -> Result<crate::OpenedPage, AdapterError> {
        Err(AdapterError::not_supported("open_url"))
    }

    /// Prepares a browser that is about to launch with `--cdp` to save its
    /// downloads in `download_dir`, before it starts. An adapter that can
    /// write the folder into the browser profile's own preference returns
    /// `Profile`; the default leaves only the DevTools connection to point
    /// downloads there.
    fn prepare_cdp_downloads(
        &self,
        _options: &crate::system::launch_options::LaunchOptions,
        _download_dir: &str,
    ) -> Result<crate::DownloadScope, AdapterError> {
        Ok(crate::DownloadScope::Connection)
    }

    /// Waits for the browser process's next download that no earlier wait
    /// returned, including one that finished before the wait began, and
    /// returns the saved file. A canceled or interrupted download is an error.
    fn wait_for_download(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<crate::DownloadInfo, AdapterError> {
        Err(AdapterError::not_supported("wait_for_download"))
    }

    fn process_state(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<crate::system::process_state::ProcessState, AdapterError> {
        Err(AdapterError::not_supported("process_state"))
    }

    fn supported_surfaces(&self) -> Vec<crate::adapter::SnapshotSurface> {
        Vec::new()
    }

    fn check_surface_support(&self, requested: crate::SnapshotSurface) -> Result<(), AdapterError> {
        let supported = self.supported_surfaces();
        if supported.contains(&requested) {
            return Ok(());
        }
        Err(requested.unavailable_error(&supported))
    }

    /// Registers caller-scoped wake hints before the initial predicate read.
    /// Without a subscription the same predicates run with bounded polling.
    fn subscribe_wait(
        &self,
        _filter: &SignalFilter,
        _deadline: Deadline,
    ) -> Result<Option<Box<dyn crate::WaitWakeup>>, AdapterError> {
        Ok(None)
    }

    /// Raises an OS-chrome surface (Start menu, Action Center, taskbar) and returns the window it
    /// presents. It takes the lease because it mutates OS chrome; the policy travels apart because
    /// the lease carries no focus-steal refusal. It refuses with `POLICY_DENIED` first when
    /// `policy.allow_focus_steal` is false, returns an already-present surface without raising it
    /// again, and answers a kind its platform lacks with a refusal naming what to ask for instead.
    fn open_system_surface(
        &self,
        _surface: crate::adapter::SnapshotSurface,
        _policy: InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("open_system_surface"))
    }

    /// Opens adapter-native connection affinity for a persistent host. The session may keep
    /// connection state but never a resolved element handle; stateless callers need none, and
    /// adapters without persistent native state return `PLATFORM_NOT_SUPPORTED`.
    fn open_session(
        &self,
        _affinity: &SessionAffinity,
        _deadline: Deadline,
    ) -> Result<Box<dyn AdapterSession>, AdapterError> {
        Err(AdapterError::not_supported("open_session"))
    }

    /// Captures a point-in-time [`SignalBaseline`], narrowed by `filter`. `deadline` bounds every
    /// native read; an adapter returns `TIMEOUT` rather than publish an observation completed at
    /// or after it. `wait --event` captures at start and on every poll and diffs them with
    /// `crate::diff_signals`: the adapter reports what the desktop looks like, never what changed.
    fn capture_signal_baseline(
        &self,
        _filter: &SignalFilter,
        _deadline: Deadline,
    ) -> Result<SignalBaseline, AdapterError> {
        Err(AdapterError::not_supported("capture_signal_baseline"))
    }

    fn close_app(
        &self,
        _app: &AppInfo,
        _force: bool,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("close_app"))
    }

    /// Reports whether closing `identifier` would end a process the OS depends on (window
    /// server, login session, shell). Each adapter owns its list; the default denies nothing.
    fn is_protected_process(&self, _identifier: &str) -> bool {
        false
    }

    /// Reports whether `combo` is a platform-dangerous shortcut refused unless the caller passes
    /// `--force` (macOS Cmd+Q quit, Ctrl+Cmd+Q lock, Cmd+Alt+Esc force-quit). Each adapter owns
    /// its list and key aliases; the default blocks nothing.
    fn is_blocked_combo(&self, _combo: &KeyCombo) -> bool {
        false
    }

    fn screenshot(
        &self,
        _target: ScreenshotTarget,
        _deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("screenshot"))
    }

    fn screenshot_window_frame(
        &self,
        _window: &WindowInfo,
        _deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("screenshot_window_frame"))
    }

    fn list_displays(&self, _deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }

    fn focused_window(&self, _deadline: Deadline) -> Result<Option<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("focused_window"))
    }

    fn press_key_for_app(
        &self,
        _process: ProcessIdentity,
        _combo: &crate::KeyCombo,
        _policy: crate::InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("press_key_for_app"))
    }

    /// Checks the exact focused window before process-scoped key delivery.
    /// Adapters can override this to retain the window identity through their
    /// native delivery gates.
    fn press_key_for_window(
        &self,
        window: &WindowInfo,
        combo: &KeyCombo,
        policy: InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        crate::adapter::window_key_delivery::press_key_for_focused_window(
            self, window, combo, policy, lease,
        )
    }

    /// Polls the menu of `process` and feeds each whole read to `wait`, which decides.
    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        _wait: crate::MenuWait,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("wait_for_menu"))
    }

    fn window_op(
        &self,
        _win: &WindowInfo,
        _op: WindowOp,
        _lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        Err(AdapterError::not_supported("window_op"))
    }

    /// Resolves the exact window id against its stored pid and process instance.
    /// Titles and bounds may change; opaque ids are interpreted only by the adapter.
    fn resolve_window_strict(
        &self,
        _win: &WindowInfo,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("resolve_window_strict"))
    }

    /// Checks runtime prerequisites without reading or opening notifications.
    /// Waits call this with an operation budget before starting their polling timeout.
    fn check_notification_support(&self, _deadline: Deadline) -> Result<(), AdapterError> {
        Ok(())
    }

    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
        _policy: InteractionPolicy,
        _deadline: Deadline,
        _lease: Option<&InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_notifications"))
    }

    fn dismiss_notification(
        &self,
        _request: DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        Err(AdapterError::not_supported("dismiss_notification"))
    }

    fn dismiss_all_notifications(
        &self,
        _request: DismissAllNotificationsRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        Err(AdapterError::not_supported("dismiss_all_notifications"))
    }

    /// `dismiss_notification` with whether the surface it opened was put back afterwards.
    fn dismiss_notification_reported(
        &self,
        request: DismissNotificationRequest<'_>,
        lease: &InteractionLease,
    ) -> Result<NotificationOutcome<NotificationInfo>, AdapterError> {
        self.dismiss_notification(request, lease)
            .map(NotificationOutcome::from)
    }

    /// `dismiss_all_notifications` with whether the surface it opened was put back afterwards.
    fn dismiss_all_notifications_reported(
        &self,
        request: DismissAllNotificationsRequest<'_>,
        lease: &InteractionLease,
    ) -> Result<DismissAllOutcome, AdapterError> {
        self.dismiss_all_notifications(request, lease)
            .map(NotificationOutcome::from)
    }

    fn notification_action(
        &self,
        _request: NotificationActionRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("notification_action"))
    }
}

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;
