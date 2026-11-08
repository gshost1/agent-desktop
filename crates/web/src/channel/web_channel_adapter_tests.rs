use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{AdapterError, ErrorCode, SnapshotSurface, SystemOps};

use super::WebChannelAdapter;
use crate::channel::link_access::LinkAccess;
use crate::channel::test_support::{
    PORT, ScriptedConnector, StubPlatform, adapter, deadline, process, targets,
};
use crate::transport::scripted_transport::ScriptedTransport;

#[test]
fn an_endpoint_the_browser_process_does_not_own_is_never_opened() {
    let connector = ScriptedConnector {
        ports: vec![PORT],
        ..ScriptedConnector::default()
    };
    let opens = connector.opens.clone();
    let web = adapter(StubPlatform::owning(PORT + 1), connector);
    let linked = web
        .with_link(&process(), LinkAccess::prompting(deadline()), |_| Ok(()))
        .unwrap();
    assert!(linked.is_none());
    assert!(opens.lock().unwrap().is_empty());
}

#[test]
fn an_owned_endpoint_is_opened_once_and_kept() {
    let transport = targets(targets(ScriptedTransport::new(), &[]), &[]);
    let connector = ScriptedConnector::with(transport);
    let opens = connector.opens.clone();
    let web = adapter(StubPlatform::owning(PORT), connector);
    for _ in 0..2 {
        let listed = web
            .with_link(&process(), LinkAccess::silent(deadline()), |link| {
                crate::list_page_targets(&mut link.connection, deadline())
            })
            .unwrap();
        assert_eq!(listed.map(|pages| pages.len()), Some(0));
    }
    assert_eq!(*opens.lock().unwrap(), vec![(PORT, false)]);
}

#[test]
fn an_unapproved_endpoint_is_refused_at_once() {
    let connector = ScriptedConnector {
        ports: vec![PORT, PORT + 1],
        open_error: Some(AdapterError::permission_denied),
        ..ScriptedConnector::default()
    };
    let opens = connector.opens.clone();
    let mut platform = StubPlatform::owning(PORT);
    platform.owned_ports.push(PORT + 1);
    let web = adapter(platform, connector);
    let error = web
        .with_link(&process(), LinkAccess::prompting(deadline()), |_| Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(*opens.lock().unwrap(), vec![(PORT, true)]);
}

#[test]
fn a_closed_connection_is_dropped_so_the_next_call_reconnects() {
    let connector = ScriptedConnector::with(ScriptedTransport::new());
    let opens = connector.opens.clone();
    let web = adapter(StubPlatform::owning(PORT), connector);
    let error = web
        .with_link(&process(), LinkAccess::silent(deadline()), |_| {
            Err::<(), _>(crate::transport::transport_error::transport_closed("gone"))
        })
        .unwrap_err();
    assert_eq!(error.details.unwrap()["kind"], "cdp_transport_closed");
    web.with_link(&process(), LinkAccess::silent(deadline()), |_| Ok(()))
        .unwrap();
    assert_eq!(opens.lock().unwrap().len(), 2);
}

#[test]
fn the_web_surface_is_supported_on_top_of_the_platform_surfaces() {
    let web = adapter(StubPlatform::default(), ScriptedConnector::default());
    assert_eq!(
        web.supported_surfaces(),
        vec![SnapshotSurface::Window, SnapshotSurface::Web]
    );
    web.check_surface_support(SnapshotSurface::Web).unwrap();
    assert_eq!(
        WebChannelAdapter::<StubPlatform>::platform_surface(SnapshotSurface::Web),
        SnapshotSurface::Window
    );
}

#[test]
fn launch_records_the_verified_endpoint_with_the_connector() {
    let connector = ScriptedConnector::default();
    let remembered = connector.remembered.clone();
    let web = adapter(StubPlatform::default(), connector);
    web.remember_cdp_endpoint(process(), &test_endpoint())
        .unwrap();
    assert_eq!(remembered.lock().unwrap()[0], (process(), PORT));
}

fn test_endpoint() -> CdpEndpoint {
    CdpEndpoint {
        port: PORT,
        http_endpoint: format!("http://127.0.0.1:{PORT}"),
        websocket_url: format!("ws://127.0.0.1:{PORT}/devtools/browser/x"),
        product: None,
        download_dir: None,
    }
}
