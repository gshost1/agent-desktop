use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use super::*;

fn app(pid: u32, name: &str) -> Running {
    Running {
        executable: keys(&[name]),
        pid,
        instance: format!("boot:{pid}"),
        name: name.to_owned(),
    }
}

fn keys(items: &[&str]) -> Option<crate::system::process::executable_identity::ExecutableIdentity> {
    Some(
        crate::system::process::executable_identity::ExecutableIdentity {
            executable: std::path::PathBuf::from(format!("/apps/{}", items[0])),
            argv: Some(vec![format!("/apps/{}", items[0]).into()]),
        },
    )
}

fn exited(code: u8) -> ExitStatus {
    ExitStatus::from_raw(i32::from(code) << 8)
}

fn killed(signal: i32) -> ExitStatus {
    ExitStatus::from_raw(signal)
}

#[test]
fn a_new_process_instance_with_a_reused_pid_is_a_new_launch_owner() {
    let mut replacement = app(7, "tool");
    replacement.instance = "boot:replacement".into();
    let before = HashSet::from([agent_desktop_core::ProcessIdentity::new(7, "boot:7")]);
    let identity = keys(&["tool"]);
    let actual = step(&Sample {
        child_pid: 100,
        child: None,
        apps: std::slice::from_ref(&replacement),
        before: &before,
        identity: &identity,
        settled: false,
    });
    assert_eq!(actual, Step::Owner(replacement));
}

#[test]
fn unreadable_owners_cannot_be_treated_as_absent_before_or_after_spawn() {
    let mut registry = Registry {
        apps: Vec::new(),
        skipped: vec![serde_json::json!({ "pid": 7, "reason": "identity unavailable" })],
    };
    let error = running_of(&registry).unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.unwrap()["skipped"][0]["pid"], 7);
    registry.skipped[0]["confirmed_gone"] = serde_json::json!(true);
    assert!(
        running_of(&registry).unwrap().is_empty(),
        "a positively observed departure is absence"
    );
}

#[test]
fn launch_observation_selects_the_owner_or_reports_the_exact_failure() {
    use Step::{Ambiguous, Failed, Owner, Wait};

    let cases = [
        (None, vec![], vec![], "tool", false, Wait),
        (
            None,
            vec![app(100, "wrapper")],
            vec![],
            "tool",
            false,
            Owner(app(100, "wrapper")),
        ),
        (None, vec![app(7, "tool")], vec![7], "tool", false, Wait),
        (
            Some(exited(0)),
            vec![app(205, "gnome-terminal-server")],
            vec![],
            "gnome-terminal-server",
            false,
            Owner(app(205, "gnome-terminal-server")),
        ),
        (
            Some(exited(0)),
            vec![app(7, "gnome-text-editor")],
            vec![7],
            "gnome-text-editor",
            false,
            Owner(app(7, "gnome-text-editor")),
        ),
        (
            Some(exited(0)),
            vec![app(7, "gnome-calculator"), app(300, "gnome-calculator")],
            vec![7],
            "gnome-calculator",
            false,
            Owner(app(300, "gnome-calculator")),
        ),
        (
            Some(exited(0)),
            vec![app(300, "tool"), app(301, "tool")],
            vec![],
            "tool",
            false,
            Ambiguous(vec![300, 301]),
        ),
        (Some(exited(0)), vec![], vec![], "tool", false, Wait),
        (
            Some(exited(0)),
            vec![],
            vec![],
            "tool",
            true,
            Failed(exited(0)),
        ),
        (
            Some(exited(3)),
            vec![],
            vec![],
            "tool",
            false,
            Failed(exited(3)),
        ),
        (
            Some(exited(2)),
            vec![app(7, "tool")],
            vec![7],
            "tool",
            true,
            Failed(exited(2)),
        ),
        (
            Some(killed(11)),
            vec![app(300, "tool")],
            vec![],
            "tool",
            true,
            Failed(killed(11)),
        ),
        (
            None,
            vec![app(7, "gnome-calculator"), app(300, "gnome-calculator")],
            vec![7],
            "gnome-calculator",
            false,
            Owner(app(300, "gnome-calculator")),
        ),
    ];

    for (index, (child, apps, before, wanted, settled, expected)) in cases.into_iter().enumerate() {
        let before = before
            .into_iter()
            .map(|pid| agent_desktop_core::ProcessIdentity::new(pid, format!("boot:{pid}")))
            .collect();
        let wanted = keys(&[wanted]);
        let actual = step(&Sample {
            child_pid: 100,
            child,
            apps: &apps,
            before: &before,
            identity: &wanted,
            settled,
        });
        assert_eq!(actual, expected, "case {index}");
    }
}

#[test]
fn a_failure_names_the_status_and_marks_the_exact_crash_disposition() {
    let error = failed(killed(9), "tool");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(error.message.contains("signal 9"));
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("details");
    assert_eq!(details["kind"], "crashed");
    assert_eq!(details["signal"], 9);

    let exited = failed(exited(4), "tool");
    assert!(exited.message.contains("status 4"));
    assert_eq!(exited.details.expect("details")["status"], 4);
}

#[test]
fn a_launcher_that_exited_is_reported_under_the_name_the_caller_asked_for() {
    let child = std::process::Command::new("/bin/true").spawn().unwrap();
    let spawned = Spawned::new(child, "gnome-calculator");
    let error = spawned.failure(exited(0));
    assert!(
        error
            .message
            .contains("'gnome-calculator' exited with status 0")
    );
    assert!(
        error
            .message
            .contains("no running instance of the executable opened a window")
    );
}

fn running_child(pid: u32) -> Spawned {
    Spawned {
        child: None,
        pid,
        identifier: "plain-x11-tool".to_owned(),
    }
}

fn empty_baseline() -> super::super::baseline::Baseline {
    super::super::baseline::Baseline {
        processes: HashSet::new(),
        residents: Vec::new(),
    }
}

fn query_with_grace<'a>(
    identity: &'a Option<crate::system::process::executable_identity::ExecutableIdentity>,
    before: &'a super::super::baseline::Baseline,
    grace: std::time::Duration,
) -> OwnerQuery<'a> {
    let deadline =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_secs(30)).unwrap();
    OwnerQuery::with_grace(identity, before, deadline, Some(grace))
}

fn stop_after_reads(
    reads: &std::cell::Cell<u32>,
    limit: u32,
    listed: Option<Vec<Running>>,
) -> impl FnMut() -> Result<Option<Vec<Running>>, AdapterError> + '_ {
    move || {
        reads.set(reads.get() + 1);
        if reads.get() > limit {
            return Err(AdapterError::internal("the owner search kept polling"));
        }
        Ok(listed.clone())
    }
}

#[test]
fn a_running_child_absent_from_a_read_registry_settles_after_the_not_on_bus_grace() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::ZERO);
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let owner = poll_owner(
        &mut spawned,
        &query,
        stop_after_reads(&reads, 3, Some(vec![app(9, "other")])),
        |_| Step::Wait,
    );
    assert_eq!(owner.unwrap(), None);
    assert_eq!(reads.get(), 1);
}

#[test]
fn the_not_on_bus_grace_waits_while_it_runs_and_ignores_unread_registries() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    for (grace, listed) in [
        (std::time::Duration::from_secs(30), Some(Vec::new())),
        (std::time::Duration::ZERO, None),
    ] {
        let query = query_with_grace(&identity, &before, grace);
        let reads = std::cell::Cell::new(0);
        let mut spawned = running_child(4242);
        let error = poll_owner(
            &mut spawned,
            &query,
            stop_after_reads(&reads, 3, listed),
            |_| Step::Wait,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Internal);
        assert_eq!(reads.get(), 4);
    }
}

#[test]
fn an_owner_found_inside_the_grace_is_the_launch_owner() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::ZERO);
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let own = app(4242, "plain-x11-tool");
    let owner = poll_owner(
        &mut spawned,
        &query,
        stop_after_reads(&reads, 3, Some(vec![own.clone()])),
        |_| Step::Wait,
    );
    assert_eq!(owner.unwrap(), Some(own));
}

#[test]
fn an_activating_launch_waits_for_the_bus_until_its_deadline() {
    let identity = None;
    let before = empty_baseline();
    let deadline =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_secs(60)).unwrap();
    let plain = OwnerQuery::new(&identity, &before, deadline, false);
    let activating = OwnerQuery::new(&identity, &before, deadline, true);
    assert!(plain.not_on_bus.remaining() <= std::time::Duration::from_secs(5));
    assert!(plain.not_on_bus.remaining() > std::time::Duration::from_secs(4));
    assert!(activating.not_on_bus.remaining() > std::time::Duration::from_secs(59));
    let short =
        agent_desktop_core::Deadline::from_duration(std::time::Duration::from_millis(500)).unwrap();
    let bounded = OwnerQuery::new(&identity, &before, short, false);
    assert!(bounded.not_on_bus.remaining() <= std::time::Duration::from_millis(500));
}

#[test]
fn a_grace_that_expires_during_a_read_is_settled_only_by_the_next_read() {
    let identity = keys(&["plain-x11-tool"]);
    let before = empty_baseline();
    let query = query_with_grace(&identity, &before, std::time::Duration::from_millis(1_000));
    let reads = std::cell::Cell::new(0);
    let mut spawned = running_child(4242);
    let owner = poll_owner(
        &mut spawned,
        &query,
        || {
            reads.set(reads.get() + 1);
            if reads.get() == 1 {
                std::thread::sleep(std::time::Duration::from_millis(1_500));
            }
            Ok(Some(Vec::new()))
        },
        |_| Step::Wait,
    );
    assert_eq!(owner.unwrap(), None);
    assert_eq!(reads.get(), 2);
}
