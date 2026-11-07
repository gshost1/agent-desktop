use agent_desktop_core::{
    AccessibilityNode, AdapterError, AppInfo, AppInventory, AppInventoryQuery, Deadline,
    HitTestResult, LiveElement, NativeHandle, ObservationOps, ObservationRequest, ObservationRoot,
    ObservedTree, PlatformAdapter, Point, PointerObservation, ProcessId, ProcessIdentity, Rect,
    RefEntry, SnapshotSurface, SurfaceInfo, TreeOptions, WindowFilter, WindowInfo,
};

use super::ref_routing::{platform_entry, web_token};
use super::web_channel_adapter::WebChannelAdapter;
use super::web_handle::WebHandle;
use super::web_observation::web_root_unsupported;
use agent_desktop_core::ElementState;

impl<P: PlatformAdapter> ObservationOps for WebChannelAdapter<P> {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface != SnapshotSurface::Web && self.inner.is_application_surface(surface)
    }

    fn focused_app(&self, deadline: Deadline) -> Result<Option<AppInfo>, AdapterError> {
        self.inner.focused_app(deadline)
    }

    fn is_nonspatial_target(&self, handle: &NativeHandle) -> bool {
        WebHandle::of(handle).is_some() || self.inner.is_nonspatial_target(handle)
    }

    fn is_nonspatial_target_with_deadline(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        if WebHandle::of(handle).is_some() {
            return Ok(true);
        }
        self.inner
            .is_nonspatial_target_with_deadline(handle, deadline)
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match root {
            ObservationRoot::Element { handle, .. } => {
                if let Some(web) = WebHandle::of(handle) {
                    return self.observe_web_element(web, &root, request);
                }
            }
            ObservationRoot::Window(window) if request.surface == SnapshotSurface::Web => {
                return self.observe_web_window(window, request);
            }
            ObservationRoot::Application { .. } if request.surface == SnapshotSurface::Web => {
                return Err(web_root_unsupported());
            }
            _ => {}
        }
        let mut platform_request = *request;
        platform_request.surface = Self::platform_surface(request.surface);
        match root {
            ObservationRoot::Element {
                handle,
                entry,
                root_ref,
            } => {
                let entry = platform_entry(entry);
                let root = ObservationRoot::Element {
                    handle,
                    entry: &entry,
                    root_ref,
                };
                self.inner.observe_tree(root, &platform_request)
            }
            root => self.inner.observe_tree(root, &platform_request),
        }
    }

    fn list_windows(
        &self,
        filter: &WindowFilter,
        deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.inner.list_windows(filter, deadline)
    }

    fn list_apps(&self, deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        self.inner.list_apps(deadline)
    }

    fn list_apps_inventory(&self, deadline: Deadline) -> Result<AppInventory, AdapterError> {
        self.inner.list_apps_inventory(deadline)
    }

    fn list_apps_scoped(
        &self,
        name: &str,
        bundle_id: Option<&str>,
        deadline: Deadline,
    ) -> Result<Vec<AppInfo>, AdapterError> {
        self.inner.list_apps_scoped(name, bundle_id, deadline)
    }

    fn query_app_inventory(
        &self,
        query: AppInventoryQuery<'_>,
        deadline: Deadline,
    ) -> Result<AppInventory, AdapterError> {
        self.inner.query_app_inventory(query, deadline)
    }

    fn get_tree(
        &self,
        win: &WindowInfo,
        opts: &TreeOptions,
        deadline: Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        self.inner.get_tree(win, opts, deadline)
    }

    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        if let Some(token) = web_token(entry) {
            return self.resolve_web_ref(entry, token, deadline);
        }
        self.inner
            .resolve_element_strict(&platform_entry(entry), deadline)
    }

    /// The platform adapter's answer. A web observation always records its
    /// element tokens, so a whole-window read the platform asks for first
    /// only reads the same page again.
    fn retains_element_tokens(&self) -> bool {
        self.inner.retains_element_tokens()
    }

    fn validate_saved_ref_context(
        &self,
        entry: &RefEntry,
        observed_token: Option<&str>,
    ) -> Result<(), AdapterError> {
        if web_token(entry).is_some() {
            return Ok(());
        }
        self.inner
            .validate_saved_ref_context(&platform_entry(entry), observed_token)
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        if let Some(token) = web_token(entry) {
            return self.resolve_web_ref(entry, token, deadline);
        }
        self.inner
            .resolve_locator_anchor(&platform_entry(entry), deadline)
    }

    fn list_surfaces(
        &self,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<Vec<SurfaceInfo>, AdapterError> {
        let mut surfaces = self.inner.list_surfaces(process.clone(), deadline)?;
        surfaces.extend(self.web_surfaces(&process, deadline));
        Ok(surfaces)
    }

    fn resolve_shell_surface(
        &self,
        surface: SnapshotSurface,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.inner.resolve_shell_surface(surface, deadline)
    }

    fn get_live_value(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => Ok(self.web_live_element(web, deadline)?.state.value),
            None => self.inner.get_live_value(handle, deadline),
        }
    }

    fn get_text_selection(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<std::ops::Range<usize>>, AdapterError> {
        match WebHandle::of(handle) {
            Some(_) => Err(AdapterError::not_supported("get_text_selection")),
            None => self.inner.get_text_selection(handle, deadline),
        }
    }

    fn get_live_state(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => Ok(Some(self.web_live_element(web, deadline)?.state)),
            None => self.inner.get_live_state(handle, deadline),
        }
    }

    fn get_live_actions(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => Ok(Some(
                self.web_live_element(web, deadline)?.available_actions,
            )),
            None => self.inner.get_live_actions(handle, deadline),
        }
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => self.web_live_element(web, deadline),
            None => self.inner.get_live_element(handle, deadline),
        }
    }

    fn get_element_bounds(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        match WebHandle::of(handle) {
            Some(_) => Ok(None),
            None => self.inner.get_element_bounds(handle, deadline),
        }
    }

    fn get_insertion_caret(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        match WebHandle::of(handle) {
            Some(_) => Ok(None),
            None => self.inner.get_insertion_caret(handle, deadline),
        }
    }

    fn get_focused_insertion_caret(
        &self,
        process: Option<ProcessId>,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        self.inner.get_focused_insertion_caret(process, deadline)
    }

    fn observe_pointer(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<PointerObservation, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => Ok(PointerObservation {
                bounds: None,
                states: self.web_live_element(web, deadline)?.state.states,
            }),
            None => self.inner.observe_pointer(handle, deadline),
        }
    }

    fn pointer_position_refusal(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Option<AdapterError> {
        match WebHandle::of(handle) {
            Some(_) => Some(super::web_refusal::no_pointer_position()),
            None => self.inner.pointer_position_refusal(handle, deadline),
        }
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: Point,
        deadline: Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        match WebHandle::of(handle) {
            Some(_) => Err(super::web_refusal::no_pointer_position()),
            None => self.inner.hit_test(handle, point, deadline),
        }
    }

    fn confirm_pointer_receiver(
        &self,
        handle: &NativeHandle,
        point: Point,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        match WebHandle::of(handle) {
            Some(_) => Err(super::web_refusal::no_pointer_position()),
            None => self.inner.confirm_pointer_receiver(handle, point, deadline),
        }
    }
}
