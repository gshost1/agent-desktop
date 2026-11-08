use std::sync::Mutex;

use super::{OpenUrlArgs, execute};
use crate::{
    AdapterError, AppError, AppInfo, Deadline, ErrorCode, OpenUrlRequest, OpenedPage,
    ProcessIdentity, WindowInfo, adapter::WindowFilter,
};

type SeenRequest = (ProcessIdentity, Option<String>, String, bool);

#[derive(Default)]
struct BrowserStub {
    windows: Vec<WindowInfo>,
    seen: Mutex<Vec<SeenRequest>>,
    opens: bool,
    lease_held_elsewhere: bool,
}

fn browser_window(id: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Docs".into(),
        app: "Browser".into(),
        pid: crate::ProcessId::new(7),
        process_instance: Some("inst-7".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            accessible: true,
            ..Default::default()
        },
    }
}

impl crate::ObservationOps for BrowserStub {
    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(self
            .windows
            .iter()
            .filter(|window| filter.app.as_deref().is_none_or(|app| app == window.app))
            .cloned()
            .collect())
    }

    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![AppInfo {
            name: "Browser".into(),
            pid: crate::ProcessId::new(7),
            process_instance: Some("inst-7".into()),
            bundle_id: None,
            presentation: None,
        }])
    }
}
impl crate::ActionOps for BrowserStub {}
impl crate::InputOps for BrowserStub {}
impl crate::SystemOps for BrowserStub {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        if self.lease_held_elsewhere {
            return Err(AdapterError::new(
                ErrorCode::Timeout,
                "another session holds the interaction lease",
            ));
        }
        crate::InteractionLease::guarded(deadline, ())
    }

    fn open_url(&self, request: &OpenUrlRequest<'_>) -> Result<OpenedPage, AdapterError> {
        if !self.opens {
            return Err(AdapterError::not_supported("open_url"));
        }
        self.seen.lock().unwrap().push((
            request.process.clone(),
            request.window.map(|window| window.id.clone()),
            request.url.to_owned(),
            request.new_tab,
        ));
        Ok(OpenedPage {
            url: request.url.into(),
            title: "Example".into(),
            channel: "cdp".into(),
            target_id: "T1".into(),
            opened: "current_tab".into(),
        })
    }
}

fn args(url: &str, new_tab: bool) -> OpenUrlArgs {
    OpenUrlArgs {
        url: url.into(),
        app: "Browser".into(),
        new_tab,
        window_id: None,
        timeout_ms: Some(2_000),
    }
}

#[test]
fn the_adapter_loads_the_url_in_the_resolved_window() {
    let stub = BrowserStub {
        windows: vec![browser_window("w1")],
        opens: true,
        ..Default::default()
    };
    let data = execute(args("https://example.com/", false), &stub).unwrap();
    assert_eq!(data["channel"], "cdp");
    assert_eq!(data["target_id"], "T1");
    assert_eq!(data["title"], "Example");
    let seen = stub.seen.lock().unwrap();
    assert_eq!(
        seen[0],
        (
            ProcessIdentity::new(7u32, "inst-7"),
            Some("w1".into()),
            "https://example.com/".into(),
            false
        )
    );
}

#[test]
fn a_new_tab_needs_no_open_window() {
    let stub = BrowserStub {
        opens: true,
        ..Default::default()
    };
    execute(args("https://example.com/", true), &stub).unwrap();
    let seen = stub.seen.lock().unwrap();
    assert_eq!(seen[0].0, ProcessIdentity::new(7u32, "inst-7"));
    assert_eq!(seen[0].1, None);
    assert!(seen[0].3);
}

#[test]
fn replacing_a_tab_needs_a_window() {
    let stub = BrowserStub {
        opens: true,
        ..Default::default()
    };
    assert!(execute(args("https://example.com/", false), &stub).is_err());
    assert!(stub.seen.lock().unwrap().is_empty());
}

#[test]
fn script_and_relative_urls_are_refused_before_the_adapter_is_asked() {
    let stub = BrowserStub {
        windows: vec![browser_window("w1")],
        opens: true,
        ..Default::default()
    };
    for url in [
        "javascript:alert(1)",
        "JavaScript:void 0",
        "example.com",
        "",
        "a b:c",
    ] {
        let error = execute(args(url, false), &stub).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidArgs.as_str(), "{url}");
    }
    assert!(stub.seen.lock().unwrap().is_empty());
}

#[test]
fn an_adapter_without_a_web_channel_reports_not_supported() {
    let stub = BrowserStub {
        windows: vec![browser_window("w1")],
        ..Default::default()
    };
    let error = execute(args("https://example.com/", false), &stub).unwrap_err();
    assert_eq!(error.code(), ErrorCode::PlatformNotSupported.as_str());
}

#[test]
fn a_page_another_session_is_driving_is_never_navigated() {
    let stub = BrowserStub {
        windows: vec![browser_window("w1")],
        opens: true,
        lease_held_elsewhere: true,
        ..Default::default()
    };
    let error = execute(args("https://example.com/", false), &stub).unwrap_err();
    assert!(stub.seen.lock().unwrap().is_empty());
    let AppError::Adapter(error) = error else {
        panic!("expected an adapter error, got {error:?}");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}
