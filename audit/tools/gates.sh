#!/usr/bin/env bash
# usage: gates.sh <worktree>   runs every AGENTS.md gate on the Linux host set
set -uo pipefail
export PATH=$HOME/.cargo/bin:$PATH
cd "$1" || exit 2
H=(-p agent-desktop-core -p agent-desktop-linux -p agent-desktop -p agent-desktop-ffi)
fail=0
step(){ echo "== $*"; "$@" >"/tmp/claude-1000/gate.$$.log" 2>&1 || { echo "FAIL: $*"; tail -40 "/tmp/claude-1000/gate.$$.log"; fail=1; }; }
step cargo fmt --all -- --check
step cargo clippy "${H[@]}" --all-targets -- -D warnings
step bash scripts/check-rust-file-size.sh
step bash scripts/check-no-phase-references.sh
step bash scripts/check-stale-ref-constructor-misuse.sh
step bash scripts/check-architecture.sh
step cargo check -p agent-desktop-core --all-targets --target x86_64-pc-windows-msvc
if cargo tree -p agent-desktop-core 2>/dev/null | grep -E 'agent-desktop-(linux|macos|windows)'; then echo "FAIL: core tree names a platform crate"; fail=1; else echo "== cargo tree clean"; fi
D=$(( 90 + RANDOM % 9 )); Xvfb :$D -screen 0 1280x800x24 >/dev/null 2>&1 & XP=$!; sleep 1
echo "== tests (DISPLAY=:$D)"
DISPLAY=:$D bash scripts/cargo-test-isolated-home.sh test "${H[@]}" >"/tmp/claude-1000/gate.$$.test.log" 2>&1 || { echo "FAIL: tests"; grep -E 'panicked|^    [a-z_:]+$|test result: FAILED' "/tmp/claude-1000/gate.$$.test.log" | head -30; fail=1; }
grep -E '^test result' "/tmp/claude-1000/gate.$$.test.log" | awk '{p+=$4; f+=$6} END {print "tests passed="p" failed="f}'
kill $XP 2>/dev/null
echo "GATES: $([ $fail = 0 ] && echo GREEN || echo RED)"
exit $fail
