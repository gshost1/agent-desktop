use crate::{
    AccessibilityNode, AdapterError, AppInfo, Deadline, ErrorCode, Rect, SnapshotSurface,
    SurfaceInfo, WindowInfo,
    adapter::live_element::LiveElement,
    adapter::native_handle::NativeHandle,
    locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
    refs::RefEntry,
    system::window_filter::WindowFilter,
    tree::element_state::ElementState,
    tree::tree_options::TreeOptions,
};

pub(crate) fn optional_live_read<T>(
    result: Result<Option<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    match result {
        Ok(value) => Ok(value),
        Err(err) if is_live_read_unsupported(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

fn is_live_read_unsupported(err: &AdapterError) -> bool {
    matches!(
        err.code,
        ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
    )
}

pub trait ObservationOps: Send + Sync {
    fn is_application_surface(&self, _surface: SnapshotSurface) -> bool {
        false
    }

    fn focused_app(&self, _deadline: Deadline) -> Result<Option<AppInfo>, AdapterError> {
        Err(AdapterError::not_supported("focused_app"))
    }

    /// Starts opening what a window-inventory read needs, such as a
    /// window-server connection, so its setup overlaps the permission
    /// preflight. A best-effort hint: callers ignore the result and no
    /// output depends on it.
    fn prepare_window_inventory(&self) -> Result<(), AdapterError> {
        Ok(())
    }

    /// Starts opening the accessibility connection to one application process
    /// a command is about to read, so its setup overlaps the permission
    /// preflight. A best-effort hint like `prepare_window_inventory`.
    fn prepare_application_connection(&self, _pid: crate::ProcessId) -> Result<(), AdapterError> {
        Ok(())
    }

    /// Reports protocol targets that have no screen position without requiring a native read.
    fn is_nonspatial_target(&self, _handle: &NativeHandle) -> bool {
        false
    }

    /// Reports proven nonspatial targets, including native menus, within the observation deadline.
    /// Semantic delivery still requires fresh identity, complete live state and action capability.
    fn is_nonspatial_target_with_deadline(
        &self,
        handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Ok(self.is_nonspatial_target(handle))
    }

    /// Whether the adapter delivers this action to this nonspatial target
    /// through its own protocol, with no pointer and no screen position, so a
    /// headless request needs no bounds and no hit test.
    fn delivers_action_without_pointer(
        &self,
        _handle: &NativeHandle,
        _action: &crate::Action,
    ) -> bool {
        false
    }

    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        Err(AdapterError::not_supported("observe_tree"))
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_windows"))
    }

    /// Whether `window` is auxiliary rather than a document window or a
    /// dialog, such as a floating tool panel or a text field's suggestion
    /// list. When `--app` matches several windows and no one of them is
    /// focused, it chooses among the windows that are not auxiliary.
    fn is_auxiliary_window(&self, _window: &WindowInfo, _deadline: Deadline) -> bool {
        false
    }

    /// The id of the one window among `windows`, all of one application, that
    /// the platform names that application's main window, asked only when
    /// `--app` still matches several. `None` when it names none, or its
    /// evidence conflicts or cannot be read.
    fn main_window(&self, _windows: &[WindowInfo], _deadline: Deadline) -> Option<String> {
        None
    }

    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_apps"))
    }

    fn list_apps_inventory(&self, deadline: Deadline) -> Result<crate::AppInventory, AdapterError> {
        Ok(crate::AppInventory {
            apps: self.list_apps(deadline)?,
            skipped: Vec::new(),
        })
    }

    fn list_apps_scoped(
        &self,
        name: &str,
        bundle_id: Option<&str>,
        deadline: Deadline,
    ) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(self
            .list_apps(deadline)?
            .into_iter()
            .filter(|app| {
                app.matches_identifier(name)
                    && bundle_id.is_none_or(|expected| {
                        app.bundle_id
                            .as_deref()
                            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
                    })
            })
            .collect())
    }

    /// Allows filtering before presentation enrichment. Skipped records describe
    /// incomplete evidence within the query; exact lookups cannot authorize
    /// mutation until that inventory is complete.
    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        query.collect(self, deadline)
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &TreeOptions,
        _deadline: Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_tree"))
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_element_strict"))
    }

    /// Whether this adapter records an element token on observed nodes, so a
    /// shallow capture can re-identify its refs without a whole-window read.
    fn retains_element_tokens(&self) -> bool {
        false
    }

    /// Checks platform-specific context that the saved ref's identity predicate
    /// does not encode, against the token from the same observation that matched
    /// it. This must not take a second live read: rooted finds and drills publish
    /// refs from that observation. Adapters without extra context accept it.
    fn validate_saved_ref_context(
        &self,
        _entry: &RefEntry,
        _observed_token: Option<&str>,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_locator_anchor"))
    }

    fn list_surfaces(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_surfaces"))
    }

    /// The surfaces an action can raise over an application: its sheets,
    /// menus, popovers and alerts, without content pages an adapter also lists
    /// as surfaces. An action reports these after it lands.
    fn list_raised_surfaces(
        &self,
        process: crate::ProcessIdentity,
        deadline: Deadline,
    ) -> Result<Vec<SurfaceInfo>, AdapterError> {
        self.list_surfaces(process, deadline)
    }

    /// Reads an already-open OS-chrome surface - the Start menu, the taskbar,
    /// the notification area, the Action Center - into the window identity
    /// the observation stack consumes. It only reads what is already up and
    /// never raises anything, which is why it takes a deadline and no
    /// interaction policy. An adapter that does not implement it answers with
    /// this default, and core's app-less surface resolution falls through to
    /// the application-owned path exactly as it did before the seam existed.
    fn resolve_shell_surface(
        &self,
        _surface: SnapshotSurface,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("resolve_shell_surface"))
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        Err(AdapterError::not_supported("get_live_value"))
    }

    /// Selected text offsets in UTF-16 code units, or unavailable evidence.
    fn get_text_selection(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<std::ops::Range<usize>>, AdapterError> {
        Err(AdapterError::not_supported("get_text_selection"))
    }

    fn get_live_state(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::not_supported("get_live_state"))
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        Err(AdapterError::not_supported("get_live_actions"))
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        Err(AdapterError::not_supported("get_live_element"))
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_element_bounds"))
    }

    /// Screen bounds of a text element's insertion point: a caret-wide rect as
    /// tall as the line it sits on. `None` when the element reports no
    /// insertion point.
    fn get_insertion_caret(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_insertion_caret"))
    }

    /// The insertion point of the text element that has keyboard focus, in
    /// `process` when given and system-wide otherwise, as
    /// [`get_insertion_caret`](Self::get_insertion_caret) reports it. `None`
    /// when the focused element is not a text element or has no insertion point.
    fn get_focused_insertion_caret(
        &self,
        _process: Option<crate::ProcessId>,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_focused_insertion_caret"))
    }

    /// Fresh pointer geometry and visibility from one observation when supported.
    /// The default preserves bounds-first reads and optional state evidence.
    fn observe_pointer(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<crate::PointerObservation, AdapterError> {
        let bounds = self.get_element_bounds(handle, deadline)?;
        let states = optional_live_read(self.get_live_state(handle, deadline))?
            .map(|state| state.states)
            .unwrap_or_default();
        Ok(crate::PointerObservation { bounds, states })
    }

    /// Why no pointer can be aimed at this target, when the adapter has proved
    /// that its screen position cannot be known, not merely that it is unread
    /// yet. Waiting cannot help, so a request that needs the position is
    /// refused with this error instead of retried until its deadline.
    fn pointer_position_refusal(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Option<AdapterError> {
        None
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: crate::Point,
        _deadline: Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        let _ = (handle, point);
        Err(AdapterError::not_supported("hit_test"))
    }

    /// Strict receiver proof for a physical pointer move that has no element
    /// action to carry it. An adapter that can prove who receives the pointer
    /// refuses with a `not_delivered` error when the target does not. `handle`
    /// is the element the caller already resolved under the interaction lease,
    /// so the adapter never re-identifies the ref by a rule of its own.
    fn confirm_pointer_receiver(
        &self,
        handle: &NativeHandle,
        point: crate::Point,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let _ = (handle, point, deadline);
        Ok(())
    }

    /// One whole read of whether `process` shows an open menu, with the same
    /// detector `SystemOps::wait_for_menu` polls.
    fn menu_open(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Err(AdapterError::not_supported("menu_open"))
    }

    /// How long a closed-direction menu wait with no sign of an open menu
    /// holds before a closed read counts: the longest an opener's menu takes
    /// to appear after the opener returns on this platform. The default is
    /// [`crate::MENU_OPENER_GRACE`]; an adapter that measured its menus
    /// appearing sooner returns its own value.
    fn menu_opener_grace(&self) -> std::time::Duration {
        crate::MENU_OPENER_GRACE
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
