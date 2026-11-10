#!/usr/bin/env bash
# usage: wt-remove.sh <worktree path>   removes a git worktree and its per-worktree cargo target dir
set -u
W=${1:?usage: wt-remove.sh <worktree path>}
NAME=$(basename "$W")
T="$HOME/.cache/agent-desktop-target/$NAME"
REPO=/home/george/Projects/agent-desktop
if [ -n "$(git -C "$W" status --porcelain 2>/dev/null)" ]; then echo "STOP: $W has uncommitted changes"; exit 1; fi
if git -C "$W" rev-parse @{u} >/dev/null 2>&1 && [ "$(git -C "$W" rev-list --count @{u}..HEAD)" != 0 ]; then echo "STOP: $W has unpushed commits"; exit 1; fi
git -C "$REPO" worktree remove "$W" && echo "removed worktree $W"
if [ -d "$T" ]; then exec 9>/tmp/claude-1000/gates.serial.lock; flock 9; find "$T" -mindepth 1 -delete && rmdir "$T" && echo "removed target dir $T"; fi
