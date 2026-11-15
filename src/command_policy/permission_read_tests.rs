use std::sync::atomic::{AtomicUsize, Ordering};

use agent_desktop_core::{
    ActionOps, AdapterError, AppError, Deadline, DeliverySemantics, ErrorCode, InputOps,
    ObservationOps, PermissionReport, PermissionState, SystemOps,
};
use clap::Parser;
use serde_json::json;

use super::read_permission_report;
use crate::cli::{Cli, Commands};
use crate::command_policy::preflight;
use crate::dispatch::pre_dispatch_error;

const DISPLAY_SUGGESTION: &str = "Start the display server, or correct DISPLAY and WAYLAND_DISPLAY";

struct ReportingAdapter {
    preflight: Result<PermissionReport, AdapterError>,
    full: PermissionReport,
    preflight_reads: AtomicUsize,
    full_reads: AtomicUsize,
}

impl ReportingAdapter {
    fn new(preflight: Result<PermissionReport, AdapterError>) -> Self {
        Self {
            preflight,
            full: denied_report(DISPLAY_SUGGESTION),
            preflight_reads: AtomicUsize::new(0),
            full_reads: AtomicUsize::new(0),
        }
    }

    fn reads(&self) -> (usize, usize) {
        (
            self.preflight_reads.load(Ordering::SeqCst),
            self.full_reads.load(Ordering::SeqCst),
        )
    }
}

impl ObservationOps for ReportingAdapter {}
impl ActionOps for ReportingAdapter {}
impl InputOps for ReportingAdapter {}
impl SystemOps for ReportingAdapter {
    fn permission_report(&self, _deadline: Deadline) -> Result<PermissionReport, AdapterError> {
        self.full_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.full.clone())
    }

    fn preflight_permission_report(
        &self,
        _deadline: Deadline,
    ) -> Result<PermissionReport, AdapterError> {
        self.preflight_reads.fetch_add(1, Ordering::SeqCst);
        self.preflight.clone()
    }
}

fn denied_report(suggestion: &str) -> PermissionReport {
    PermissionReport {
        accessibility: PermissionState::Denied {
            suggestion: suggestion.into(),
        },
        screen_recording: PermissionState::Unknown,
        automation: PermissionState::NotRequired,
    }
}

fn display_refusal() -> AdapterError {
    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        "The session's display server is not running: DISPLAY=:1987 names a server that is not running",
    )
    .with_suggestion(DISPLAY_SUGGESTION)
    .with_details(json!({
        "display_unavailable": "missing",
        "detail": "DISPLAY=:1987 names a server that is not running",
        "retryable": false,
        "bus_error": { "message": "The AT-SPI accessibility bus is not available in this session" },
    }))
    .with_platform_detail("org.freedesktop.DBus.Error.NoReply: Message recipient disconnected")
}

fn command(arguments: &[&str]) -> Commands {
    let argv = std::iter::once("agent-desktop").chain(arguments.iter().copied());
    let Ok(cli) = Cli::try_parse_from(argv) else {
        panic!("{arguments:?} parses");
    };
    let Some(command) = cli.command else {
        panic!("{arguments:?} names a subcommand");
    };
    command
}

/// What the binary answers before dispatch: the report read, then the
/// permission preflight, as `execute_with_adapter` runs them.
fn answer(arguments: &[&str], adapter: &ReportingAdapter) -> Result<(), AdapterError> {
    let cmd = command(arguments);
    read_permission_report(&cmd, adapter)
        .map_err(pre_dispatch_error)
        .and_then(|report| preflight(&cmd, &report))
        .map_err(AppError::into_adapter_error)
}

const PREFLIGHTED: [&[&str]; 3] = [
    &["launch", "/usr/bin/true"],
    &["snapshot", "--app", "Finder"],
    &["click", "@s8f3k2p9:e1"],
];

#[test]
fn an_adapter_refusal_of_the_preflight_read_is_the_answer_unchanged() {
    let expected = display_refusal();
    for arguments in PREFLIGHTED {
        let adapter = ReportingAdapter::new(Err(display_refusal()));
        let Err(error) = answer(arguments, &adapter) else {
            panic!("{arguments:?} must be refused");
        };
        assert_eq!(error.code, ErrorCode::PlatformNotSupported, "{arguments:?}");
        assert_eq!(error.message, expected.message, "{arguments:?}");
        assert_eq!(error.suggestion, expected.suggestion, "{arguments:?}");
        assert_eq!(error.details, expected.details, "{arguments:?}");
        assert_eq!(error.platform_detail, expected.platform_detail);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
        assert_eq!(
            adapter.reads(),
            (1, 0),
            "{arguments:?} reads the preflight report"
        );
    }
}

#[test]
fn a_denied_preflight_report_stays_perm_denied() {
    for arguments in PREFLIGHTED {
        let adapter = ReportingAdapter::new(Ok(denied_report("Grant Accessibility")));
        let Err(error) = answer(arguments, &adapter) else {
            panic!("{arguments:?} must be refused");
        };
        assert_eq!(error.code, ErrorCode::PermDenied, "{arguments:?}");
        assert_eq!(error.suggestion.as_deref(), Some("Grant Accessibility"));
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn a_command_that_needs_no_permission_never_reads_a_report() {
    for arguments in [&["list-apps"][..], &["list-displays"]] {
        let adapter = ReportingAdapter::new(Err(display_refusal()));
        assert!(answer(arguments, &adapter).is_ok(), "{arguments:?}");
        assert_eq!(adapter.reads(), (0, 0), "{arguments:?}");
    }
}

#[test]
fn status_reads_the_full_report_and_keeps_a_denied_bus_as_state() {
    let adapter = ReportingAdapter::new(Err(display_refusal()));
    let report = read_permission_report(&command(&["status"]), &adapter)
        .expect("status reads the full report");
    assert_eq!(report, denied_report(DISPLAY_SUGGESTION));
    assert_eq!(adapter.reads(), (0, 1));
}
