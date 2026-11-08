#!/usr/bin/env bash
# Runs the macOS E2E harness in each PR worktree (each already contains fork main).
# bench/guard blocks the harness's System Events window-resize call; Automation for
# System Events is denied here, so that step fails on every branch alike. The
# harness needs Python 3.10+ as python3; macOS ships 3.9 at /usr/bin/python3.
cd "$(dirname "$0")/.."
root="$PWD"
python_dir="$root/bench/out/e2e-python"
mkdir -p "$python_dir"; ln -sf "$(command -v python3.11 || command -v python3.12 || command -v python3.13)" "$python_dir/python3"
"$python_dir/python3" -c 'import sys; sys.exit(sys.version_info < (3, 10))' || { echo "E2E needs Python 3.10 or newer as python3"; exit 1; }
export PATH="$root/bench/guard:$python_dir:$PATH"
git fetch -q origin
for wt in wt-surface wt-cold wt-popup wt-aeprobe; do
    d="$root/bench/out/$wt"
    [ -d "$d" ] || continue
    (cd "$d" && AGENT_DESKTOP_E2E_EXCLUSIVE=1 bash tests/e2e/run.sh > "$root/bench/out/e2e-$wt.log" 2>&1)
    echo "$wt e2e exit $? $(tail -3 "$root/bench/out/e2e-$wt.log" | tr '\n' ' ')"
done | tee "$root/bench/out/e2e-branches.txt"
