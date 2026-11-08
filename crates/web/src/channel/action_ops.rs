use agent_desktop_core::{
    ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, HeadedRequirement,
    InteractionLease, NativeHandle, PlatformAdapter, RefEntry,
};

use super::ref_routing::{platform_entry, web_token};
use super::web_channel_adapter::WebChannelAdapter;
use super::web_handle::WebHandle;

impl<P: PlatformAdapter> ActionOps for WebChannelAdapter<P> {
    /// A web ref needs no focused window and no cursor: it is driven through
    /// the page in the background, headed or not.
    fn headed_requirement_for_ref(
        &self,
        entry: &RefEntry,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<HeadedRequirement, AdapterError> {
        if web_token(entry).is_some() {
            return Ok(HeadedRequirement::None);
        }
        self.inner
            .headed_requirement_for_ref(&platform_entry(entry), request, deadline)
    }

    fn headed_requirement(
        &self,
        handle: &NativeHandle,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<HeadedRequirement, AdapterError> {
        if WebHandle::of(handle).is_some() {
            return Ok(HeadedRequirement::None);
        }
        self.inner.headed_requirement(handle, request, deadline)
    }

    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => self.execute_web_action(web, request, lease),
            None => self.inner.execute_action(handle, request, lease),
        }
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => self.scroll_web_into_view(web, lease.deadline()),
            None => self.inner.scroll_into_view(handle, lease),
        }
    }

    fn scroll_ref_into_view(
        &self,
        entry: &RefEntry,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<Option<NativeHandle>, AdapterError> {
        match WebHandle::of(handle) {
            Some(web) => self
                .scroll_web_into_view(web, lease.deadline())
                .map(|()| None),
            None => self
                .inner
                .scroll_ref_into_view(&platform_entry(entry), handle, lease),
        }
    }
}

impl<P: PlatformAdapter> WebChannelAdapter<P> {
    fn scroll_web_into_view(
        &self,
        web: &WebHandle,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.with_required_link(&web.process, deadline, |link| {
            let session = link.session(&web.token.target_id, deadline)?;
            crate::scroll_into_view(&mut link.connection, &session, &web.token, deadline)
        })
    }
}
