#!/usr/bin/env bash
# Builds the two checkouts of the skill A/B: perf/skill-quickstart and its merge
# base with main, so the skill text is the only difference.
cd "$(dirname "$0")/.."
git fetch -q origin
base=$(git merge-base origin/main origin/perf/skill-quickstart)
for pair in "wt-skillbase:$base" "wt-skill:origin/perf/skill-quickstart"; do
    dir="bench/out/${pair%%:*}"; rev="${pair#*:}"
    [ -d "$dir" ] || git worktree add -q --detach "$dir" "$rev"
    git -C "$dir" checkout -q --detach "$rev"
    (cd "$dir" && nice -n 10 cargo build --release -q -p agent-desktop) || { echo "build failed in $dir" >&2; exit 1; }
    echo "$dir $(git -C "$dir" rev-parse --short HEAD) skill $(wc -c < "$dir/skills/agent-desktop/SKILL.md") B"
done | tee bench/out/skill-ab-builds.txt
