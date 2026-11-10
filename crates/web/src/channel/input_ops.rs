use agent_desktop_core::{
    AdapterError, ClipboardContent, ClipboardFormat, Deadline, DragParams, InputOps,
    InteractionLease, KeyCombo, MouseEvent, OperationReport, PlatformAdapter,
};

use super::web_channel_adapter::WebChannelAdapter;

impl<P: PlatformAdapter> InputOps for WebChannelAdapter<P> {
    fn physical_pointer_refusal(&self) -> Option<AdapterError> {
        self.inner.physical_pointer_refusal()
    }

    fn keyboard_needs_window_activation(&self) -> bool {
        self.inner.keyboard_needs_window_activation()
    }

    fn prepare_keyboard_input(&self) -> Result<(), AdapterError> {
        self.inner.prepare_keyboard_input()
    }

    fn mouse_event(
        &self,
        event: MouseEvent,
        lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.inner.mouse_event(event, lease)
    }

    fn key_event(
        &self,
        combo: &KeyCombo,
        down: bool,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.inner.key_event(combo, down, lease)
    }

    fn drag(
        &self,
        params: DragParams,
        lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.inner.drag(params, lease)
    }

    fn clear_clipboard(&self, lease: &InteractionLease) -> Result<OperationReport, AdapterError> {
        self.inner.clear_clipboard(lease)
    }

    fn get_clipboard_content(
        &self,
        format: ClipboardFormat,
        deadline: Deadline,
    ) -> Result<Option<ClipboardContent>, AdapterError> {
        self.inner.get_clipboard_content(format, deadline)
    }

    fn set_clipboard_content(
        &self,
        content: &ClipboardContent,
        lease: &InteractionLease,
    ) -> Result<OperationReport, AdapterError> {
        self.inner.set_clipboard_content(content, lease)
    }
}

#[cfg(test)]
#[path = "input_ops_tests.rs"]
mod tests;
