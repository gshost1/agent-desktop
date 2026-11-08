#!/usr/bin/env bash
# Builds the three checkouts the headline A/B runs on, and records what each holds.
#   wt-upstream      upstream main at the fork point, b43ac4f6
#   wt-fork          fork main; any of the fix PRs it does not contain yet is merged
#                    in locally, and the list of those is recorded
#   wt-fork-outline  wt-fork plus perf/outline, for the batch-first + --outline variant
# Writes bench/out/headline-builds.json.
set -u
cd "$(dirname "$0")/.."
source bench/cargo_env.sh
root="$PWD"
FIXES="perf/settled-surface-budget perf/cold-settle-chromium fix/popup-window-viewport fix/automation-probe-deadline fix/open-closed-sheet"
git fetch -q origin
checkout() {
    local dir="$1" rev="$2"
    [ -d "$dir" ] || git worktree add -q --detach "$dir" "$rev"
    git -C "$dir" merge --abort 2>/dev/null
    git -C "$dir" checkout -q --detach "$rev"
}
merge_missing() {
    local dir="$1"; shift
    local merged=""
    for branch in "$@"; do
        git -C "$dir" merge-base --is-ancestor "origin/$branch" HEAD && continue
        git -C "$dir" -c user.name=bench -c user.email=bench@localhost merge -q --no-edit "origin/$branch" >/dev/null 2>&1 \
            || { echo "merge of $branch into $dir failed" >&2; git -C "$dir" merge --abort; exit 1; }
        merged="$merged $branch"
    done
    echo "$merged"
}
checkout bench/out/wt-upstream b43ac4f6
checkout bench/out/wt-fork origin/main
fork_local=$(merge_missing bench/out/wt-fork $FIXES)
checkout bench/out/wt-fork-outline "$(git -C bench/out/wt-fork rev-parse HEAD)"
outline_local=$(merge_missing bench/out/wt-fork-outline perf/outline)
for dir in wt-upstream wt-fork wt-fork-outline; do
    bench_build "bench/out/$dir" || { echo "build failed in $dir" >&2; exit 1; }
done
python3 - "$fork_local" "$outline_local" <<'PY'
import json, subprocess, sys, time
def head(d): return subprocess.run(["git", "-C", f"bench/out/{d}", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
info = {
    "since": time.strftime("%m%d%H%M%S"),
    "upstream": {"tree": "bench/out/wt-upstream", "head": head("wt-upstream"), "base": "b43ac4f6"},
    "fork": {"tree": "bench/out/wt-fork", "head": head("wt-fork"),
             "main": subprocess.run(["git", "rev-parse", "--short", "origin/main"], capture_output=True, text=True).stdout.strip(),
             "merged_locally": sys.argv[1].split()},
    "fork_outline": {"tree": "bench/out/wt-fork-outline", "head": head("wt-fork-outline"), "merged_locally": sys.argv[2].split()},
}
json.dump(info, open("bench/out/headline-builds.json", "w"), indent=1)
print(json.dumps(info))
PY
