use super::*;
use agent_desktop_core::system::launch_presentation::LaunchPresentation;

fn options(attach: bool, activate: bool, reopen: bool) -> LaunchOptions {
    LaunchOptions {
        attach_if_running: attach,
        presentation: LaunchPresentation { activate, reopen },
        ..Default::default()
    }
}

#[test]
fn a_running_app_is_activated_without_launch_services() {
    assert_eq!(
        plan_launch(&options(true, true, false), 1),
        LaunchPlan::ActivateRunning
    );
}

#[test]
fn a_running_app_without_activate_is_only_attached() {
    assert_eq!(
        plan_launch(&options(true, false, false), 1),
        LaunchPlan::AttachRunning
    );
}

#[test]
fn reopen_sends_a_running_app_through_launch_services() {
    for activate in [false, true] {
        assert_eq!(
            plan_launch(&options(true, activate, true), 1),
            LaunchPlan::LaunchServices(LaunchPath::Reopened)
        );
    }
}

#[test]
fn an_app_that_is_not_running_is_launched() {
    for activate in [false, true] {
        for reopen in [false, true] {
            assert_eq!(
                plan_launch(&options(true, activate, reopen), 0),
                LaunchPlan::LaunchServices(LaunchPath::Launched)
            );
        }
    }
}

#[test]
fn a_fresh_instance_is_launched_even_when_the_app_is_running() {
    for activate in [false, true] {
        assert_eq!(
            plan_launch(&options(false, activate, false), 1),
            LaunchPlan::LaunchServices(LaunchPath::Launched)
        );
    }
}

#[test]
fn several_running_matches_reach_the_ambiguity_check() {
    for activate in [false, true] {
        assert_eq!(
            plan_launch(&options(true, activate, false), 2),
            LaunchPlan::LaunchServices(LaunchPath::Launched)
        );
    }
}
