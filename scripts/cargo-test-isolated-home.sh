#!/usr/bin/env bash
#
# Runs cargo with a throwaway HOME, so tests never touch ~/.agent-desktop, and
# cut off from the caller's desktop session, so a test that reaches for the
# session bus or the accessibility bus never lands in a logged-in user's real
# session.
#
# By default the script unsets WAYLAND_DISPLAY, AT_SPI_BUS_ADDRESS and
# DBUS_SESSION_BUS_PID, points DBUS_SESSION_BUS_ADDRESS at a socket path that
# does not exist, and gives cargo a private XDG_RUNTIME_DIR (mode 700). Both
# temporary directories are removed when cargo exits. DISPLAY is kept: the CI
# lanes and the local gates hand in a private Xvfb display on purpose.
#
# AGENT_DESKTOP_TEST_SESSION=inherit opts a run back into the caller's session
# bus, Wayland display and runtime directory, for a caller that already set up
# a private session (dbus-run-session, scripts/private-session.sh) and wants the
# tests to use it. Any other value than `isolate` (the default) or `inherit` is
# refused with exit code 2.

set -euo pipefail

original_home="${HOME:?HOME is required}"
cargo_home="${CARGO_HOME:-$original_home/.cargo}"
rustup_home="${RUSTUP_HOME:-$original_home/.rustup}"
session_mode="${AGENT_DESKTOP_TEST_SESSION:-isolate}"

case "$session_mode" in
    isolate | inherit) ;;
    *)
        printf 'cargo-test-isolated-home: AGENT_DESKTOP_TEST_SESSION must be isolate or inherit, got "%s"\n' \
            "$session_mode" >&2
        exit 2
        ;;
esac

test_home="$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-test-home.XXXXXX")"
runtime_dir=""

cleanup() {
    rm -rf -- "$test_home"
    if [ -n "$runtime_dir" ]; then
        rm -rf -- "$runtime_dir"
    fi
}
trap cleanup EXIT

if [ "$session_mode" = isolate ]; then
    runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-test-runtime.XXXXXX")"
    chmod 700 "$runtime_dir"
    unset WAYLAND_DISPLAY AT_SPI_BUS_ADDRESS DBUS_SESSION_BUS_PID
    export DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_dir/no-session-bus"
    export XDG_RUNTIME_DIR="$runtime_dir"
fi

HOME="$test_home" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" cargo "$@"
