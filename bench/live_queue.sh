#!/usr/bin/env bash
# One desktop session for the live checks, in the control room's order:
# #18 and #14 checks, the TextEdit save-folder check, the #24 and #25 probes
# (under mac-verify), then the #26 probe (takes its own lock). Bench folders only.
set -u
cd "$(dirname "$0")/.."
out=bench/out/live-$(date +%m%d%H%M%S); mkdir -p "$out"
export AGENT_DESKTOP_HOME="$PWD/$out/ad-home"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
step() { echo "== $1 $(date +%H:%M:%S)"; }
bash bench/lockq.sh take "mac-verify" 5
bash bench/wait_builds_idle.sh
step pr18; python3 bench/mac_verify.py pr18 bench/out/wt-pr18/target/release/agent-desktop > "$out/pr18.txt" 2>&1; tail -3 "$out/pr18.txt"
step pr14; python3 bench/mac_verify.py pr14 bench/out/wt-pr14/target/release/agent-desktop bench/out/fixture/AgentDeskFixture.app > "$out/pr14.txt" 2>&1; tail -3 "$out/pr14.txt"
step save-folder; python3 bench/check_save_folder.py bench/out/wt-pr14/target/release/agent-desktop > "$out/save-folder.txt" 2>&1; tail -1 "$out/save-folder.txt"
step pr24; bash "$HOME/agent-desktop-bench/probe-bin-inline-editor/probe_inline_editor.sh" > "$out/pr24.txt" 2>&1; tail -2 "$out/pr24.txt"
step pr25; bash /private/tmp/claude-501/-Users-Shost2--t3-worktrees-agent-desktop-t3-c50fa083/93a977ec-79bb-4570-9563-1a56e0704017/scratchpad/probe-press-menu.sh > "$out/pr25.txt" 2>&1; tail -2 "$out/pr25.txt"
bash bench/lockq.sh release "mac-verify"
step pr26; bash "$HOME/.agent-desktop-save-panel-probe/probe.sh" > "$out/pr26.txt" 2>&1; tail -2 "$out/pr26.txt"
echo "done $out"
