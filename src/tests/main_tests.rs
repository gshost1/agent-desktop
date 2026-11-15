use agent_desktop_core::{AdapterError, AppError, DeliveryDisposition, RetryDisposition};
use clap::Parser;

#[test]
fn visual_debug_flags_accept_snapshot_and_click() {
    for args in [
        vec![
            "agent-desktop",
            "snapshot",
            "--app",
            "TextEdit",
            "--skeleton",
            "-i",
            "--debug",
            "--screenshot",
            "/tmp/skeleton.html",
        ],
        vec![
            "agent-desktop",
            "click",
            "@sdemo:e1",
            "--debug",
            "--screenshot",
            "/tmp/click.html",
        ],
    ] {
        assert!(crate::Cli::try_parse_from(args).is_ok());
    }
}

#[test]
fn visual_debug_flags_require_explicit_capture_consent_and_path() {
    for flags in [vec!["--debug"], vec!["--screenshot", "/tmp/debug.html"]] {
        let mut args = vec!["agent-desktop", "snapshot"];
        args.extend(flags);
        assert!(crate::Cli::try_parse_from(args).is_err());
    }
}

#[path = "batch_seen_set_tests.rs"]
mod batch_seen_set_tests;

#[test]
fn pre_dispatch_failures_are_always_safe_to_retry() {
    for error in [
        AppError::from(AdapterError::internal("trace setup failed")),
        AppError::from(std::io::Error::other("read failed")),
    ] {
        let AppError::Adapter(error) = crate::pre_dispatch_error(error) else {
            panic!("pre-dispatch error must be normalized to AdapterError");
        };
        assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::NotDelivered
        );
    }
}

#[test]
fn a_top_level_batch_preflight_gets_the_batch_budget_capped_not_five_seconds() {
    let deadline_of = |args: &[&str]| {
        let cli = crate::Cli::try_parse_from(args).expect("parses");
        crate::preflight_deadline(&cli.command.expect("a command"))
            .expect("a deadline builds")
            .remaining()
    };
    let batch = deadline_of(&["agent-desktop", "batch", "[]", "--timeout-ms", "60000"]);
    assert!(batch > std::time::Duration::from_secs(10), "{batch:?}");
    assert!(
        batch <= std::time::Duration::from_millis(15_000),
        "{batch:?}"
    );
    let short = deadline_of(&["agent-desktop", "batch", "[]", "--timeout-ms", "2000"]);
    assert!(
        short <= std::time::Duration::from_millis(2_000),
        "{short:?}"
    );
    let status = deadline_of(&["agent-desktop", "status"]);
    assert!(
        status <= std::time::Duration::from_millis(5_000),
        "{status:?}"
    );
}
