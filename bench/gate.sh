#!/usr/bin/env bash
# Runs the local merge gate for each named commit in one gate worktree
# (bench/out/wt-gate, detached) against the shared bench target dir, so every
# gate rebuilds incrementally in one place instead of filling a target/ per
# branch worktree.
#
# Usage: bash bench/gate.sh <label>=<rev> ...   (one line per rev in bench/out/gate.txt)
set -u
cd "$(dirname "$0")/.."
root="$PWD"
source bench/cargo_env.sh
gate="$root/bench/out/wt-gate"
[ -d "$gate" ] || git worktree add -q --detach "$gate" origin/main
HOST_PKGS=(-p agent-desktop-core -p agent-desktop-macos -p agent-desktop -p agent-desktop-ffi)
for pair in "$@"; do
    label="${pair%%=*}"; rev="${pair#*=}"
    log="$root/bench/out/gate-$label.log"
    git -C "$gate" checkout -q --detach "$rev" || { echo "$label $rev checkout failed" | tee -a bench/out/gate.txt; continue; }
    (
        cd "$gate"
        set -o pipefail
        bash scripts/check-rust-file-size.sh && bash scripts/check-no-phase-references.sh \
            && bash scripts/check-architecture.sh && cargo fmt --all -- --check \
            && bash "$root/bench/serial_cargo.sh" cargo clippy "${HOST_PKGS[@]}" --all-targets -- -D warnings \
            && bash "$root/bench/serial_cargo.sh" bash scripts/cargo-test-isolated-home.sh test "${HOST_PKGS[@]}"
    ) > "$log" 2>&1
    status=$?
    passed=$(grep -o '[0-9]* passed' "$log" | awk '{s+=$1} END {print s+0}')
    failed=$(grep -o '[0-9]* failed' "$log" | awk '{s+=$1} END {print s+0}')
    echo "$label $(git -C "$gate" rev-parse --short HEAD) exit=$status passed=$passed failed=$failed" | tee -a bench/out/gate.txt
done
