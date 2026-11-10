#!/usr/bin/env bash
exec 9>/tmp/claude-1000/gates.serial.lock; flock 9
# usage: gates.sh <worktree>   runs every AGENTS.md gate on the Linux host set
set -uo pipefail
export PATH=$HOME/.cargo/bin:$PATH
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
cd "$1" || exit 2
export CARGO_TARGET_DIR="$HOME/.cache/agent-desktop-target/$(basename "$(git rev-parse --show-toplevel)")"
echo "== target dir: $CARGO_TARGET_DIR"
unset WAYLAND_DISPLAY AT_SPI_BUS_ADDRESS DBUS_SYSTEM_BUS_ADDRESS
export DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/agent-desktop-gates-bus
GATES_RT=$(mktemp -d /tmp/claude-1000/gates-rt.XXXXXX) && chmod 700 "$GATES_RT" && export XDG_RUNTIME_DIR="$GATES_RT"
echo "== isolated from the desktop session: no WAYLAND_DISPLAY, no session bus, XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"
H=(-p agent-desktop-core -p agent-desktop-linux -p agent-desktop -p agent-desktop-ffi)
[ -d crates/web ] && H+=(-p agent-desktop-web)
echo "== host set: ${H[*]}"
fail=0
step(){ echo "== $*"; "$@" >"/tmp/claude-1000/gate.$$.log" 2>&1 || { echo "FAIL: $*"; tail -40 "/tmp/claude-1000/gate.$$.log"; fail=1; }; }
step cargo fmt --all -- --check
step cargo clippy "${H[@]}" --all-targets -- -D warnings
step bash scripts/check-rust-file-size.sh
step bash scripts/check-no-phase-references.sh
step bash scripts/check-stale-ref-constructor-misuse.sh
step bash scripts/check-architecture.sh
step cargo check -p agent-desktop-core -p agent-desktop-ffi --all-targets --target x86_64-pc-windows-msvc
if grep -rnE 'extern "system|std::os::windows|\bwinapi\b|\bwindows_sys\b' crates/core/src; then echo "FAIL: core declares a raw Win32 surface"; fail=1; fi
if grep -rnE 'cfg\(windows\)|cfg\(target_os = "windows"\)' crates/core/src | grep -v 'crates/core/src/storage/private_file.rs'; then echo "FAIL: windows cfg in core outside private_file.rs"; fail=1; else echo "== core win32 leakage check clean"; fi
if cargo tree -p agent-desktop-core 2>/dev/null | grep -E 'agent-desktop-(linux|macos|windows)'; then echo "FAIL: core tree names a platform crate"; fail=1; else echo "== cargo tree clean"; fi
D=""; for n in $(shuf -i 100-199); do if [ ! -e /tmp/.X11-unix/X$n ] && [ ! -e /tmp/.X$n-lock ]; then D=$n; break; fi; done
Xvfb :$D -screen 0 1280x800x24 9>&- >/dev/null 2>&1 & XP=$!; trap 'kill $XP 2>/dev/null; find "$GATES_RT" -mindepth 1 -delete 2>/dev/null; rmdir "$GATES_RT" 2>/dev/null' EXIT; for _ in 1 2 3 4 5 6 7 8 9 10; do [ -e /tmp/.X11-unix/X$D ] && break; sleep 0.5; done
echo "== tests (DISPLAY=:$D)"
DISPLAY=:$D bash scripts/cargo-test-isolated-home.sh test --no-fail-fast "${H[@]}" >"/tmp/claude-1000/gate.$$.test.log" 2>&1 || { echo "FAIL: tests"; grep -E 'panicked|^    [a-z_:]+$|test result: FAILED' "/tmp/claude-1000/gate.$$.test.log" | head -30; fail=1; }
grep -E '^test result' "/tmp/claude-1000/gate.$$.test.log" | awk '{p+=$4; f+=$6} END {print "tests passed="p" failed="f}'
kill $XP 2>/dev/null
echo "GATES: $([ $fail = 0 ] && echo GREEN || echo RED)"
exit $fail
