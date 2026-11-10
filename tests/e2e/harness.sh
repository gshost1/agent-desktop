#!/usr/bin/env bash

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tests/e2e/python.sh
source "$repo/tests/e2e/python.sh"
json_tool="$repo/tests/e2e/json_tool.py"
command_guard="$repo/tests/e2e/guard-command.sh"
release_bin="${AGENT_DESKTOP_E2E_RELEASE_BIN:-$repo/target/release/agent-desktop}"
release_ffi="${AGENT_DESKTOP_E2E_RELEASE_FFI:-$repo/target/release-ffi/libagent_desktop_ffi.dylib}"
release_ffi_helper="${AGENT_DESKTOP_E2E_RELEASE_FFI_HELPER:-$repo/target/release/agent-desktop-macos-helper}"
host_home="${HOME:-}"
host_tmp="${TMPDIR:-/tmp}"
suite_root=""
lock_owned=0
prepared_binary=""
prepared_binary_sha=""
copied_file_sha=""

require_exclusive_acknowledgement() {
    if [ "${AGENT_DESKTOP_E2E_EXCLUSIVE:-}" != "1" ]; then
        echo "SKIP (blocked): native E2E requires an exclusive desktop" >&2
        echo "Set AGENT_DESKTOP_E2E_EXCLUSIVE=1 only after closing competing automation and stopping user input." >&2
        return 1
    fi
}

acquire_exclusive_lock() {
    case "${AGENT_DESKTOP_INTERACTION_LEASE_FD:-}" in
        ''|*[!0-9]*)
            echo "SKIP (blocked): canonical inherited interaction lease FD is missing" >&2
            return 1
            ;;
    esac
    if ! "$e2e_python" "$repo/tests/e2e/interaction_lock.py" verify \
        "$AGENT_DESKTOP_INTERACTION_LEASE_FD"; then
        echo "SKIP (blocked): inherited interaction lease FD is invalid" >&2
        return 1
    fi
    lock_owned=1
}

release_exclusive_lock() {
    lock_owned=0
}

setup_isolated_environment() {
    local label="$1"
    suite_root="$(mktemp -d "/tmp/ad-${label}.XXXXXX")" || return 1
    suite_root="$(cd "$suite_root" && pwd -P)" || return 1
    printf '%s\n' "$$" > "$suite_root/.agent-desktop-e2e-root"
    mkdir -p "$suite_root/home" "$suite_root/tmp" "$suite_root/bin" "$suite_root/fixture"
    chmod 700 "$suite_root/tmp"
    export CARGO_HOME="${CARGO_HOME:-$host_home/.cargo}"
    export RUSTUP_HOME="${RUSTUP_HOME:-$host_home/.rustup}"
    export HOME="$suite_root/home"
    export TMPDIR="$suite_root/tmp"
    export XDG_CACHE_HOME="$suite_root/home/.cache"
    export XDG_CONFIG_HOME="$suite_root/home/.config"
    export XDG_DATA_HOME="$suite_root/home/.local/share"
    export CARGO_TARGET_DIR="${AGENT_DESKTOP_E2E_CARGO_TARGET_DIR:-$suite_root/cargo-target}"
    export AGENT_DESKTOP_E2E_TIMEOUT_SECONDS="${AGENT_DESKTOP_E2E_TIMEOUT_SECONDS:-20}"
    export AGENT_DESKTOP_E2E_MAX_CAPTURE_BYTES="${AGENT_DESKTOP_E2E_MAX_CAPTURE_BYTES:-2097152}"
    unset AGENT_DESKTOP_SESSION
}

cleanup_isolated_environment() {
    if [ -z "$suite_root" ] || [ ! -d "$suite_root" ]; then
        return
    fi
    local owner=""
    if [ -f "$suite_root/.agent-desktop-e2e-root" ]; then
        owner="$(sed -n '1p' "$suite_root/.agent-desktop-e2e-root" 2>/dev/null)"
    fi
    if [ "$owner" = "$$" ]; then
        remove_artifact "$suite_root" || true
    else
        echo "refusing to remove unowned E2E directory: $suite_root" >&2
    fi
}

remove_artifact() {
    local path="$1"
    if [ -z "$path" ]; then
        echo "cleanup refused an empty path" >&2
        return 1
    fi
    if [ ! -e "$path" ] && [ ! -L "$path" ]; then
        return 0
    fi
    if ! trash "$path"; then
        echo "cleanup failed; retained artifact: $path" >&2
        return 1
    fi
    if [ -e "$path" ] || [ -L "$path" ]; then
        echo "cleanup did not remove artifact; retained: $path" >&2
        return 1
    fi
}

copy_immutable_binary() {
    local source="$1"
    local label="$2"
    if [ ! -x "$source" ]; then
        echo "executable binary missing at $source" >&2
        return 1
    fi
    local source_before source_after destination copy_hash
    source_before="$(shasum -a 256 "$source" | awk '{print $1}')" || return 1
    destination="$suite_root/bin/$label"
    cp "$source" "$destination" || return 1
    source_after="$(shasum -a 256 "$source" | awk '{print $1}')" || return 1
    copy_hash="$(shasum -a 256 "$destination" | awk '{print $1}')" || return 1
    if [ "$source_before" != "$source_after" ] || [ "$source_before" != "$copy_hash" ]; then
        echo "binary changed while it was copied; refusing a contaminated run" >&2
        return 1
    fi
    chmod 500 "$destination"
    prepared_binary="$destination"
    prepared_binary_sha="$copy_hash"
}

verify_immutable_binary() {
    local binary="$1"
    local expected_sha="$2"
    if [ ! -f "$binary" ]; then
        echo "immutable E2E binary disappeared: $binary" >&2
        return 1
    fi
    local actual_sha
    actual_sha="$(shasum -a 256 "$binary" | awk '{print $1}')" || return 1
    if [ "$actual_sha" != "$expected_sha" ]; then
        echo "immutable E2E binary changed during the run" >&2
        return 1
    fi
}

copy_native_ffi_bundle() {
    if [ ! -f "$release_ffi" ]; then
        echo "release FFI library missing at $release_ffi" >&2
        return 1
    fi
    if [ ! -x "$release_ffi_helper" ]; then
        echo "release FFI helper missing at $release_ffi_helper" >&2
        return 1
    fi
    local stage="$suite_root/ffi"
    mkdir -p "$stage"
    copy_immutable_file "$release_ffi" "$stage/libagent_desktop_ffi.dylib" || return 1
    local dylib_sha="$copied_file_sha"
    copy_immutable_file "$release_ffi_helper" "$stage/agent-desktop-macos-helper" || return 1
    local helper_sha="$copied_file_sha"
    chmod 500 "$stage/libagent_desktop_ffi.dylib" "$stage/agent-desktop-macos-helper"
    export AGENT_DESKTOP_E2E_FFI_DYLIB="$stage/libagent_desktop_ffi.dylib"
    export AGENT_DESKTOP_E2E_FFI_DYLIB_SHA
    export AGENT_DESKTOP_E2E_FFI_HELPER_SHA
    AGENT_DESKTOP_E2E_FFI_DYLIB_SHA="$dylib_sha"
    AGENT_DESKTOP_E2E_FFI_HELPER_SHA="$helper_sha"
}

copy_immutable_file() {
    local source="$1" destination="$2"
    local source_before source_after destination_hash
    source_before="$(shasum -a 256 "$source" | awk '{print $1}')" || return 1
    cp "$source" "$destination" || return 1
    source_after="$(shasum -a 256 "$source" | awk '{print $1}')" || return 1
    destination_hash="$(shasum -a 256 "$destination" | awk '{print $1}')" || return 1
    if [ "$source_before" != "$source_after" ] || [ "$source_before" != "$destination_hash" ]; then
        echo "artifact changed while it was copied; refusing a contaminated run" >&2
        return 1
    fi
    copied_file_sha="$destination_hash"
}

fixture_min_width=980
fixture_min_height=748

fixture_frame_from_displays() {
    "$e2e_python" -c '
import json, sys
width = int(sys.argv[1]) if sys.argv[1] else 1000
d = json.load(sys.stdin)["data"]
best = max(d, key=lambda x: x["bounds"]["width"] * x["bounds"]["height"])
b = best["bounds"]
if int(b["width"]) - 40 < width and sys.argv[1]:
    sys.exit(3)
w = min(int(b["width"]) - 40, width)
h = min(int(b["height"]) - 88, 1080)
print(",".join(str(v) for v in (int(b["x"]) + 20, int(b["y"]) + 64, w, h)))
' "${1:-}" 2>/dev/null
}

fixture_frame_meets_minimum() {
    "$e2e_python" -c '
import sys
parts = sys.argv[1].split(",")
ok = len(parts) == 4 and float(parts[2]) >= int(sys.argv[2]) and float(parts[3]) >= int(sys.argv[3])
sys.exit(0 if ok else 1)
' "$1" "$fixture_min_width" "$fixture_min_height" 2>/dev/null
}

require_display_fits_fixture() {
    local frame
    frame="$("$1" list-displays 2>/dev/null | fixture_frame_from_displays)"
    if [ -z "$frame" ]; then
        echo "SKIP (blocked): display bounds are unreadable, so the fixture window frame cannot be normalized" >&2
        return 1
    fi
    if ! fixture_frame_meets_minimum "$frame"; then
        echo "SKIP (blocked): the largest display leaves a fixture frame of $frame (x,y,width,height); the scenarios need at least ${fixture_min_width}x${fixture_min_height} points. Use a display of at least $((fixture_min_width + 40))x$((fixture_min_height + 88)) points." >&2
        return 1
    fi
}

guard_exec() {
    local timeout_seconds="$1"
    local max_capture_bytes="$2"
    shift 2
    AGENT_DESKTOP_E2E_TIMEOUT_SECONDS="$timeout_seconds" \
        AGENT_DESKTOP_E2E_MAX_CAPTURE_BYTES="$max_capture_bytes" \
        "$e2e_python" "$json_tool" exec "$@"
}

guard_agent_wrapper_exec() {
    local timeout_seconds="$1"
    local max_capture_bytes="$2"
    shift 2
    AGENT_DESKTOP_E2E_TIMEOUT_SECONDS="$timeout_seconds" \
        AGENT_DESKTOP_E2E_MAX_CAPTURE_BYTES="$max_capture_bytes" \
        AGENT_DESKTOP_E2E_INHERIT_LEASE=1 \
        "$e2e_python" "$json_tool" exec "$@"
}
