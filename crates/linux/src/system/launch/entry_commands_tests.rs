use super::*;

const EDITOR: &str = "[Desktop Entry]\nType=Application\nName=Editor\nExec=editor %U\nActions=new-window;missing;empty;\nDBusActivatable=true\n\n[Desktop Action new-window]\nName=New Window\nExec=editor --new-window\n\n[Desktop Action empty]\nName=Empty\n\n[Desktop Action unlisted]\nExec=editor --unlisted\n";

fn data_dir(name: &str, service: Option<&str>) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(crate::tree::live::live_launch::nonce(name));
    let applications = root.join("applications");
    std::fs::create_dir_all(&applications).unwrap();
    if let Some(text) = service {
        let services = root.join(SERVICES);
        std::fs::create_dir_all(&services).unwrap();
        std::fs::write(services.join("org.example.Editor.service"), text).unwrap();
    }
    (root, applications)
}

#[test]
fn only_listed_actions_with_an_exec_add_a_command() {
    assert_eq!(action_execs(EDITOR), ["editor --new-window"]);
    assert!(action_execs("[Desktop Entry]\nName=Plain\nExec=plain\n").is_empty());
}

#[test]
fn an_activatable_entry_adds_the_command_of_its_own_bus_service() {
    let (root, applications) = data_dir(
        "entry-service",
        Some(
            "[D-BUS Service]\nName=org.example.Editor\nExec=/usr/bin/editor --gapplication-service\n",
        ),
    );
    let dirs = [root.join("missing/applications"), applications];
    assert_eq!(
        activation_exec("org.example.Editor", EDITOR, &dirs).as_deref(),
        Some("/usr/bin/editor --gapplication-service")
    );
    assert_eq!(
        other_execs("org.example.Editor.desktop", EDITOR, &dirs),
        [
            "editor --new-window",
            "/usr/bin/editor --gapplication-service"
        ]
    );
    let plain = EDITOR.replace("DBusActivatable=true", "DBusActivatable=false");
    assert_eq!(activation_exec("org.example.Editor", &plain, &dirs), None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_service_file_that_names_another_bus_name_is_not_the_entry_s() {
    let (root, applications) = data_dir(
        "entry-service-other",
        Some("[D-BUS Service]\nName=org.example.Other\nExec=/usr/bin/other\n"),
    );
    assert_eq!(
        activation_exec("org.example.Editor", EDITOR, &[applications]),
        None
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_id_that_could_name_another_file_reads_no_service() {
    for id in ["../org.example.Editor", "a/b", "", ".."] {
        assert_eq!(bus_name(id), None, "{id}");
    }
    assert_eq!(
        bus_name("org.example.Editor.desktop"),
        Some("org.example.Editor")
    );
}

#[test]
fn a_service_the_bus_starts_as_a_systemd_unit_adds_no_command() {
    let (root, applications) = data_dir(
        "entry-service-systemd",
        Some(
            "[D-BUS Service]\nName=org.example.Editor\nExec=/bin/false\nSystemdService=editor.service\n",
        ),
    );
    assert_eq!(
        activation_exec("org.example.Editor", EDITOR, &[applications]),
        None
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_service_file_change_changes_the_service_stamps() {
    let (root, applications) = data_dir(
        "entry-service-stamp",
        Some("[D-BUS Service]\nName=org.example.Editor\nExec=/usr/bin/editor\n"),
    );
    let dirs = [applications];
    let before = service_stamps(&dirs);
    assert_eq!(before.len(), 2);
    std::fs::write(
        root.join(SERVICES).join("org.example.Editor.service"),
        "[D-BUS Service]\nName=org.example.Editor\nExec=/usr/bin/editor --other\n",
    )
    .unwrap();
    assert!(before != service_stamps(&dirs));
    std::fs::remove_dir_all(root).unwrap();
}
