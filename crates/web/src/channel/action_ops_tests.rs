use std::sync::Mutex;

use agent_desktop_core::{
    ActionOps, AdapterError, Direction, ErrorCode, InputOps, InteractionLease, NativeHandle,
    ObservationOps, ScrollPage, SystemOps,
};

use super::WebChannelAdapter;
use crate::ElementToken;
use crate::channel::test_support::{ScriptedConnector, TOKEN, lease, process};
use crate::channel::web_handle::WebHandle;

#[derive(Default)]
struct PagingPlatform {
    pages: Mutex<usize>,
}

impl ObservationOps for PagingPlatform {}
impl ActionOps for PagingPlatform {
    fn scroll_page(
        &self,
        _handle: &NativeHandle,
        _direction: &Direction,
        _lease: &InteractionLease,
    ) -> Result<ScrollPage, AdapterError> {
        *self.pages.lock().unwrap() += 1;
        Ok(ScrollPage::Moved)
    }
}
impl InputOps for PagingPlatform {}
impl SystemOps for PagingPlatform {}

#[test]
fn a_native_scroll_page_reaches_the_platform_and_a_web_one_is_refused_unsent() {
    let connector = ScriptedConnector::default();
    let opens = connector.opens.clone();
    let web = WebChannelAdapter::with_connector(PagingPlatform::default(), Box::new(connector))
        .with_download_ledger(None);
    let native = web.scroll_page(&NativeHandle::new("native"), &Direction::Down, &lease());
    assert_eq!(native.unwrap(), ScrollPage::Moved);
    let web_ref = NativeHandle::new(WebHandle {
        process: process(),
        token: ElementToken::parse(TOKEN).unwrap(),
    });
    let refused = web
        .scroll_page(&web_ref, &Direction::Down, &lease())
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::PlatformNotSupported);
    assert_eq!(*web.inner().pages.lock().unwrap(), 1);
    assert!(opens.lock().unwrap().is_empty());
}
