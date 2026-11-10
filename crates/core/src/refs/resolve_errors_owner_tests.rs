use crate::ErrorCode;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::resolve_saved_ref::test_support::{RuleAdapter, entry, node, tree_with};

struct GoneOwner {
    inner: RuleAdapter,
    state: Option<crate::system::process_state::ProcessState>,
}

impl GoneOwner {
    fn new(state: Option<crate::system::process_state::ProcessState>) -> Self {
        Self {
            inner: RuleAdapter::new(tree_with(vec![node("button", "Run")])),
            state,
        }
    }
}

impl crate::adapter::ObservationOps for GoneOwner {
    fn list_apps_scoped(
        &self,
        _name: &str,
        _bundle_id: Option<&str>,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::AppInfo>, crate::AdapterError> {
        Ok(Vec::new())
    }

    fn list_windows(
        &self,
        filter: &crate::adapter::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, crate::AdapterError> {
        match &filter.app {
            Some(app) => Err(crate::AdapterError::new(
                ErrorCode::AppNotFound,
                format!("No running application matches '{app}'"),
            )),
            None => Ok(Vec::new()),
        }
    }

    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, crate::AdapterError> {
        self.inner.observe_tree(root, request)
    }
}

impl crate::adapter::ActionOps for GoneOwner {}
impl crate::adapter::InputOps for GoneOwner {}
impl crate::adapter::SystemOps for GoneOwner {
    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<crate::system::process_state::ProcessState, crate::AdapterError> {
        self.state
            .ok_or_else(|| crate::AdapterError::not_supported("process_state"))
    }
}

fn resolve_gone(adapter: &GoneOwner, window_id: Option<&str>) -> crate::AdapterError {
    let mut entry = entry();
    entry.source.source_window_id = window_id.map(str::to_owned);
    entry.process.process_instance = Some("test-instance".into());
    match crate::refs::resolve_saved_ref::resolve_saved_ref(
        adapter,
        &entry,
        crate::Deadline::standard().unwrap(),
        None,
    ) {
        Ok(_) => panic!("a ref of an application that is not found cannot resolve"),
        Err(error) => error,
    }
}

#[test]
fn a_ref_whose_application_exited_is_stale_and_names_the_exit() {
    let _guard = HomeGuard::new();
    let exited = GoneOwner::new(Some(crate::system::process_state::ProcessState::Exited {
        code: None,
    }));

    for window_id in [Some("w-1"), None] {
        let error = resolve_gone(&exited, window_id);

        assert_eq!(error.code, ErrorCode::StaleRef, "window id {window_id:?}");
        assert!(!error.permits_retry_by_default());
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "saved_ref_absent");
        assert_eq!(details["process_state"], "exited");
        assert_eq!(details["complete"], true);
        assert_eq!(details["retryable"], false);
    }
}

#[test]
fn an_application_not_found_while_its_process_runs_or_is_unknown_stays_app_not_found() {
    let _guard = HomeGuard::new();
    for state in [
        Some(crate::system::process_state::ProcessState::Running),
        Some(crate::system::process_state::ProcessState::Unresponsive),
        None,
    ] {
        let adapter = GoneOwner::new(state);

        let error = resolve_gone(&adapter, Some("w-1"));

        assert_eq!(
            error.code,
            ErrorCode::AppNotFound,
            "process state {state:?}"
        );
    }
}
