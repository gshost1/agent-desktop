#!/usr/bin/env bash
# macOS E2E on fork main (the merge base of the open fix PRs), run the same way as
# bench/e2e_branches.sh, for the no-regression baseline: guard against System
# Events, python3.11 first on PATH.
cd "$(dirname "$0")/.."
root="$PWD"
d="$root/bench/out/wt-main"
git -C "$d" fetch -q origin && git -C "$d" checkout -q --detach origin/main
python_dir="$root/bench/out/e2e-python"
mkdir -p "$python_dir"; ln -sf "$(command -v python3.11 || command -v python3.12 || command -v python3.13)" "$python_dir/python3"
export PATH="$root/bench/guard:$python_dir:$PATH"
(cd "$d" && AGENT_DESKTOP_E2E_EXCLUSIVE=1 bash tests/e2e/run.sh > "$root/bench/out/e2e-wt-main.log" 2>&1)
echo "wt-main $(git -C "$d" rev-parse --short HEAD) e2e exit $? $(tail -3 "$root/bench/out/e2e-wt-main.log" | tr '\n' ' ')" | tee "$root/bench/out/e2e-main.txt"
