#!/usr/bin/env bash
# Bench round prelude: make the desktop Space current (a full-screen app in front hides
# windows that a plain launch opens), then quit the app it used. Outside measured time.
set -u
B=/Users/Shost2/.t3/worktrees/agent-desktop/bench-v130/target/release/agent-desktop
export AGENT_DESKTOP_HOME=$(mktemp -d /private/tmp/claude-501/-Users-Shost2--t3-worktrees-agent-desktop-t3-7c029123/6c037495-151e-453b-a8c7-33efd49f72b5/scratchpad/space.XXXX); chmod 700 "$AGENT_DESKTOP_HOME"
"$B" launch Calculator --activate >/dev/null 2>&1
/Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/out/macctl quit Calculator >/dev/null 2>&1
sleep 1
rm -rf "$AGENT_DESKTOP_HOME"
