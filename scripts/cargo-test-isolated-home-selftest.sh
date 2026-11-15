#!/usr/bin/env bash
#
# Proves scripts/cargo-test-isolated-home.sh in both directions with a stub
# `cargo` on PATH that records the environment it was given. By default the
# stub must see a throwaway HOME, no WAYLAND_DISPLAY or AT_SPI_BUS_ADDRESS, a
# session bus address whose socket does not exist, and a private runtime
# directory that is gone once the script returns. With
# AGENT_DESKTOP_TEST_SESSION=inherit it must see the caller's session unchanged.
# An unknown mode must be refused, and cargo's exit code must come back.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/scripts/cargo-test-isolated-home.sh"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/isolated-home-selftest.XXXXXX")"
trap 'rm -rf -- "$WORK"' EXIT

fail() {
    echo "self-test FAIL: $1" >&2
    exit 1
}

mkdir -p "$WORK/bin" "$WORK/caller-runtime"
cat >"$WORK/bin/cargo" <<'STUB'
#!/usr/bin/env bash
{
    printf 'HOME=%s\n' "$HOME"
    printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY-<unset>}"
    printf 'AT_SPI_BUS_ADDRESS=%s\n' "${AT_SPI_BUS_ADDRESS-<unset>}"
    printf 'DBUS_SESSION_BUS_PID=%s\n' "${DBUS_SESSION_BUS_PID-<unset>}"
    printf 'DBUS_SESSION_BUS_ADDRESS=%s\n' "${DBUS_SESSION_BUS_ADDRESS-<unset>}"
    printf 'XDG_RUNTIME_DIR=%s\n' "${XDG_RUNTIME_DIR-<unset>}"
    printf 'DISPLAY=%s\n' "${DISPLAY-<unset>}"
    printf 'ARGS=%s\n' "$*"
    if [ -n "${XDG_RUNTIME_DIR-}" ] && [ -d "$XDG_RUNTIME_DIR" ]; then
        printf 'RUNTIME_MODE=%s\n' "$(ls -ld "$XDG_RUNTIME_DIR" | cut -c1-10)"
    fi
} >"$STUB_RECORD"
exit "${STUB_EXIT:-0}"
STUB
chmod +x "$WORK/bin/cargo"

run_script() {
    env PATH="$WORK/bin:$PATH" STUB_RECORD="$WORK/record" \
        HOME="$WORK/caller-home" \
        WAYLAND_DISPLAY=wayland-caller \
        AT_SPI_BUS_ADDRESS=unix:path=/caller/at-spi \
        DBUS_SESSION_BUS_PID=4242 \
        DBUS_SESSION_BUS_ADDRESS=unix:path=/caller/bus \
        XDG_RUNTIME_DIR="$WORK/caller-runtime" \
        DISPLAY=:4242 \
        "$@"
}

value_of() {
    sed -n "s/^$1=//p" "$WORK/record"
}

expect() {
    local name=$1 want=$2 got
    got="$(value_of "$name")"
    [ "$got" = "$want" ] || fail "$3: $name is '$got', want '$want'"
}

mkdir -p "$WORK/caller-home"

run_script bash "$SCRIPT" test --lib || fail "default run failed"
expect WAYLAND_DISPLAY '<unset>' "default run"
expect AT_SPI_BUS_ADDRESS '<unset>' "default run"
expect DBUS_SESSION_BUS_PID '<unset>' "default run"
expect DISPLAY ':4242' "default run"
expect ARGS 'test --lib' "default run"
expect RUNTIME_MODE 'drwx------' "default run"
runtime="$(value_of XDG_RUNTIME_DIR)"
home="$(value_of HOME)"
bus="$(value_of DBUS_SESSION_BUS_ADDRESS)"
case "$runtime" in
    "$WORK/caller-runtime" | /run/user/* | '') fail "default run kept a session runtime dir: '$runtime'" ;;
esac
[ "$home" != "$WORK/caller-home" ] || fail "default run kept the caller's HOME"
[ "$bus" = "unix:path=$runtime/no-session-bus" ] || fail "default run bus is '$bus'"
[ ! -e "$runtime" ] || fail "private runtime dir $runtime survived the run"
[ ! -e "$home" ] || fail "throwaway HOME $home survived the run"

run_script env AGENT_DESKTOP_TEST_SESSION=isolate bash "$SCRIPT" test || fail "isolate run failed"
expect WAYLAND_DISPLAY '<unset>' "explicit isolate run"

run_script env AGENT_DESKTOP_TEST_SESSION=inherit bash "$SCRIPT" test || fail "inherit run failed"
expect WAYLAND_DISPLAY 'wayland-caller' "inherit run"
expect AT_SPI_BUS_ADDRESS 'unix:path=/caller/at-spi' "inherit run"
expect DBUS_SESSION_BUS_PID '4242' "inherit run"
expect DBUS_SESSION_BUS_ADDRESS 'unix:path=/caller/bus' "inherit run"
expect XDG_RUNTIME_DIR "$WORK/caller-runtime" "inherit run"
[ "$(value_of HOME)" != "$WORK/caller-home" ] || fail "inherit run kept the caller's HOME"
[ -d "$WORK/caller-runtime" ] || fail "inherit run removed the caller's runtime dir"

rm -f "$WORK/record"
got=0
run_script env AGENT_DESKTOP_TEST_SESSION=desktop bash "$SCRIPT" test 2>/dev/null || got=$?
[ "$got" -eq 2 ] || fail "an unknown mode exited $got, want 2"
[ ! -e "$WORK/record" ] || fail "an unknown mode still ran cargo"

got=0
run_script env STUB_EXIT=101 bash "$SCRIPT" test || got=$?
[ "$got" -eq 101 ] || fail "cargo exit 101 came back as $got"
runtime="$(value_of XDG_RUNTIME_DIR)"
[ ! -e "$runtime" ] || fail "private runtime dir $runtime survived a failed run"

echo "OK: cargo-test-isolated-home.sh isolates the desktop session and honours the opt-in"
