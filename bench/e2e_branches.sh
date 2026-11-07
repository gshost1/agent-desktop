#!/usr/bin/env bash
# Runs the macOS E2E harness in each PR worktree (each already contains fork main).
# bench/guard blocks the harness's System Events window-resize call; Automation for
# System Events is denied here, so that step fails on every branch alike.
cd "$(dirname "$0")/.."
root="$PWD"
export PATH="$root/bench/guard:$PATH"
git fetch -q origin
for wt in wt-surface wt-cold wt-popup wt-aeprobe; do
    d="$root/bench/out/$wt"
    [ -d "$d" ] || continue
    (cd "$d" && AGENT_DESKTOP_E2E_EXCLUSIVE=1 bash tests/e2e/run.sh > "$root/bench/out/e2e-$wt.log" 2>&1)
    echo "$wt e2e exit $? $(tail -3 "$root/bench/out/e2e-$wt.log" | tr '\n' ' ')"
done | tee "$root/bench/out/e2e-branches.txt"
