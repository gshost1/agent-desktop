use std::sync::Mutex;

use serde_json::Value;

use super::cdp_test_support::{CdpAdapter, empty_cdp_adapter};
use super::{LaunchArgs, execute};
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::system::cdp_endpoint::CdpEndpoint;
use crate::system::launch_options::LaunchOptions;
use crate::{AdapterError, AppInfo, DownloadScope, ErrorCode, InteractionLease, ProcessIdentity};

struct ProfileAdapter {
    inner: CdpAdapter,
    refuses: bool,
    prepared: Mutex<Vec<(Vec<String>, String)>>,
    remembered: Mutex<Option<CdpEndpoint>>,
}

impl ObservationOps for ProfileAdapter {
    fn list_apps(&self, deadline: crate::Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        self.inner.list_apps(deadline)
    }
}
impl ActionOps for ProfileAdapter {}
impl InputOps for ProfileAdapter {}

impl SystemOps for ProfileAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        self.inner.acquire_interaction_lease(deadline)
    }

    fn launch_app(
        &self,
        id: &str,
        options: &LaunchOptions,
        lease: &InteractionLease,
    ) -> Result<crate::system::launch_result::LaunchResult, AdapterError> {
        assert_eq!(
            self.prepared.lock().unwrap().len(),
            1,
            "prepared before launch"
        );
        self.inner.launch_app(id, options, lease)
    }

    fn process_owns_tcp_listener(
        &self,
        process: ProcessIdentity,
        port: u16,
        deadline: crate::Deadline,
    ) -> Result<bool, AdapterError> {
        self.inner
            .process_owns_tcp_listener(process, port, deadline)
    }

    fn prepare_cdp_downloads(
        &self,
        options: &LaunchOptions,
        download_dir: &str,
    ) -> Result<DownloadScope, AdapterError> {
        self.prepared
            .lock()
            .unwrap()
            .push((options.args.clone(), download_dir.to_owned()));
        if self.refuses {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "read-only profile",
            ));
        }
        Ok(DownloadScope::Profile)
    }

    fn remember_cdp_endpoint(
        &self,
        _process: ProcessIdentity,
        endpoint: &CdpEndpoint,
    ) -> Result<(), AdapterError> {
        *self.remembered.lock().unwrap() = Some(endpoint.clone());
        Ok(())
    }
}

fn adapter(refuses: bool) -> ProfileAdapter {
    ProfileAdapter {
        inner: empty_cdp_adapter(true),
        refuses,
        prepared: Mutex::new(Vec::new()),
        remembered: Mutex::new(None),
    }
}

fn launch(adapter: &ProfileAdapter, dir: &std::path::Path) -> Value {
    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions {
                cdp_port: Some(0),
                timeout_ms: 3_000,
                args: vec!["--user-data-dir=/tmp/profile".into()],
                ..Default::default()
            },
            download_dir: Some(dir.to_path_buf()),
        },
        adapter,
    )
    .unwrap()
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ad-launch-dl-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_download_folder_is_written_into_the_profile_before_launch_and_reported() {
    let adapter = adapter(false);
    let dir = temp_dir("profile");

    let value = launch(&adapter, &dir);

    let prepared = adapter.prepared.lock().unwrap();
    assert_eq!(prepared[0].0[0], "--user-data-dir=/tmp/profile");
    assert_eq!(prepared[0].1, dir.to_string_lossy());
    assert_eq!(value["cdp"]["download_scope"], "profile");
    let remembered = adapter.remembered.lock().unwrap().clone().unwrap();
    assert_eq!(remembered.download_scope, Some(DownloadScope::Profile));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_profile_that_refuses_the_folder_leaves_it_to_each_connection() {
    let adapter = adapter(true);
    let dir = temp_dir("refused");

    let value = launch(&adapter, &dir);

    assert_eq!(value["cdp"]["download_scope"], "connection");
    assert_eq!(value["cdp"]["download_dir"], dir.to_string_lossy().as_ref());
    let _ = std::fs::remove_dir_all(&dir);
}
