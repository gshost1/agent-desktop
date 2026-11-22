use super::{EnvSubset, is_relevant};

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

#[test]
fn display_bus_state_root_and_agent_desktop_settings_are_relevant() {
    for name in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
        "AT_SPI_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
        "AGENT_DESKTOP_HOME",
        "AGENT_DESKTOP_SESSION",
        "AGENT_DESKTOP_DAEMON_IDLE_MS",
        "HOME",
        "PATH",
        "LC_ALL",
        "RUST_LOG",
    ] {
        assert!(is_relevant(name), "{name} must key the daemon");
    }
}

#[test]
fn shell_noise_and_the_daemon_switches_are_not_relevant() {
    for name in [
        "AGENT_DESKTOP_DAEMON",
        "AGENT_DESKTOP_DAEMON_SERVE",
        "PWD",
        "OLDPWD",
        "SHLVL",
        "_",
    ] {
        assert!(!is_relevant(name), "{name} must not key the daemon");
    }
}

#[test]
fn subsets_compare_by_value_regardless_of_order() {
    let one = EnvSubset::from_pairs(vec![pair("DISPLAY", ":1"), pair("HOME", "/h")]);
    let same = EnvSubset::from_pairs(vec![pair("HOME", "/h"), pair("DISPLAY", ":1")]);
    let other = EnvSubset::from_pairs(vec![pair("HOME", "/h"), pair("DISPLAY", ":2")]);

    assert_eq!(one, same);
    assert_ne!(one, other);
}

#[test]
fn irrelevant_pairs_are_dropped() {
    let subset = EnvSubset::from_pairs(vec![pair("PWD", "/tmp"), pair("DISPLAY", ":1")]);

    assert_eq!(subset.pairs(), &[pair("DISPLAY", ":1")]);
}
