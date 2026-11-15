use std::collections::HashSet;

use agent_desktop_core::ProcessId;

use super::*;

fn identity(argv: &[&str]) -> Option<ExecutableIdentity> {
    Some(ExecutableIdentity {
        executable: std::path::PathBuf::from(argv[0]),
        argv: Some(argv.iter().map(Into::into).collect()),
    })
}

fn running(pid: u32, argv: &[&str]) -> Running {
    Running {
        executable: identity(argv),
        pid,
        instance: format!("boot:{pid}"),
        name: "app".to_owned(),
    }
}

fn baseline(residents: &[Running], instances: usize) -> Baseline {
    Baseline {
        processes: residents.iter().map(Running::identity).collect(),
        residents: residents
            .iter()
            .map(|resident| (resident.clone(), HashSet::from(["old".to_owned()])))
            .collect(),
        instances,
    }
}

fn chosen(before: &Baseline, request: &[&str], apps: &[Running], serves: bool) -> Step {
    choose(before, &identity(request), apps, |_| true, |_| serves)
}

#[test]
fn the_one_serving_instance_is_adopted() {
    let settings = running(7, &["/usr/bin/settings"]);
    let before = baseline(std::slice::from_ref(&settings), 1);
    assert_eq!(
        chosen(
            &before,
            &["/usr/bin/settings", "datetime"],
            std::slice::from_ref(&settings),
            true
        ),
        Step::Owner(settings)
    );
}

#[test]
fn an_instance_that_owns_no_launch_interface_is_not_adopted() {
    let python = running(7, &["/usr/bin/python3"]);
    let before = baseline(std::slice::from_ref(&python), 1);
    assert_eq!(
        chosen(&before, &["/usr/bin/python3", "tool.py"], &[python], false),
        Step::Wait
    );
}

#[test]
fn an_interpreter_running_another_script_is_not_adopted_even_when_it_serves() {
    let resident = running(7, &["/usr/bin/python3", "-u", "other.py"]);
    let before = baseline(std::slice::from_ref(&resident), 1);
    for request in [&["/usr/bin/python3", "tool.py"][..], &["/usr/bin/python3"]] {
        assert_eq!(
            chosen(&before, request, std::slice::from_ref(&resident), true),
            Step::Wait,
            "{request:?}"
        );
    }
    assert_eq!(
        chosen(
            &before,
            &["/usr/bin/python3", "other.py", "--page"],
            std::slice::from_ref(&resident),
            true
        ),
        Step::Owner(resident)
    );
}

#[test]
fn a_second_instance_unread_before_the_spawn_blocks_adoption() {
    let first = running(7, &["/usr/bin/settings"]);
    let before = baseline(std::slice::from_ref(&first), 2);
    assert_eq!(
        chosen(
            &before,
            &["/usr/bin/settings"],
            &[first, running(8, &["/usr/bin/settings"])],
            true
        ),
        Step::Wait
    );
}

#[test]
fn two_read_instances_or_one_gone_or_unread_now_block_adoption() {
    let first = running(7, &["/usr/bin/settings"]);
    let second = running(8, &["/usr/bin/settings"]);
    let both = baseline(&[first.clone(), second.clone()], 2);
    assert_eq!(
        chosen(
            &both,
            &["/usr/bin/settings"],
            std::slice::from_ref(&first),
            true
        ),
        Step::Wait
    );
    let one = baseline(std::slice::from_ref(&first), 1);
    assert_eq!(chosen(&one, &["/usr/bin/settings"], &[], true), Step::Wait);
    assert_eq!(
        choose(
            &one,
            &identity(&["/usr/bin/settings"]),
            &[first],
            |_| false,
            |_| true
        ),
        Step::Wait
    );
}

fn window(pid: u32, instance: &str, id: &str) -> WindowInfo {
    WindowInfo {
        id: id.to_owned(),
        title: id.to_owned(),
        app: "app".to_owned(),
        pid: ProcessId::new(pid),
        process_instance: Some(instance.to_owned()),
        bounds: None,
        state: agent_desktop_core::WindowState::default(),
    }
}

#[test]
fn an_adopted_instance_reports_its_only_window_and_none_of_several() {
    let owner = running(7, &["/usr/bin/settings"]);
    let only = window(7, "boot:7", "w1");
    let other = window(9, "boot:9", "w9");
    let id = |found: Option<WindowInfo>| found.map(|window| window.id);
    assert_eq!(
        id(only_window(&owner, vec![only.clone(), other.clone()])),
        Some("w1".to_owned())
    );
    assert_eq!(
        id(only_window(
            &owner,
            vec![only, window(7, "boot:7", "w2"), other]
        )),
        None
    );
    assert!(only_window(&owner, Vec::new()).is_none());
}

#[test]
fn the_adoption_note_names_the_instance_and_keeps_an_earlier_note() {
    let note = adoption_note(7, None);
    assert!(note.contains("running instance (pid 7)"), "{note}");
    assert!(note.contains("already open"), "{note}");
    assert!(note.contains("not verified"), "{note}");
    let joined = adoption_note(
        7,
        Some("The window could not be brought forward: x".to_owned()),
    );
    assert!(
        joined.ends_with("The window could not be brought forward: x"),
        "{joined}"
    );
}

#[test]
fn a_launch_interface_is_read_from_the_application_object() {
    assert_eq!(
        bus::application_path("org.gnome.Settings"),
        "/org/gnome/Settings"
    );
    assert_eq!(
        bus::application_path("org.example.my-app"),
        "/org/example/my_app"
    );
    assert!(bus::declares_launch_interface(
        "<node><interface name=\"org.gtk.Application\"><method name=\"Activate\"/></interface></node>"
    ));
    assert!(bus::declares_launch_interface(
        "<node><interface name=\"org.freedesktop.Application\"></interface></node>"
    ));
    assert!(!bus::declares_launch_interface(
        "<node><interface name=\"org.mpris.MediaPlayer2\"></interface></node>"
    ));
}

#[test]
fn an_option_value_before_the_script_is_part_of_the_resident_operands() {
    let resident = running(7, &["/usr/bin/python3", "-W", "ignore", "a.py"]);
    let before = baseline(std::slice::from_ref(&resident), 1);
    let apps = std::slice::from_ref(&resident);
    assert_eq!(
        chosen(
            &before,
            &["/usr/bin/python3", "-W", "ignore", "b.py"],
            apps,
            true
        ),
        Step::Wait
    );
    assert_eq!(
        chosen(
            &before,
            &["/usr/bin/python3", "-W", "ignore", "a.py", "page2"],
            apps,
            true
        ),
        Step::Owner(resident.clone())
    );
    let java = running(8, &["/usr/bin/java", "-cp", "x.jar", "A"]);
    let before = baseline(std::slice::from_ref(&java), 1);
    assert_eq!(
        chosen(
            &before,
            &["/usr/bin/java", "-cp", "x.jar", "B"],
            std::slice::from_ref(&java),
            true
        ),
        Step::Wait
    );
}
