#!/bin/bash
# Non-GUI checks before the held-out run. Reads files, versions and permission status only; opens no
# window and drives no app. Exit 1 when anything an arm needs is missing.
set -u
H=$(cd "$(dirname "$0")" && pwd)
P=${HELDOUT_PY:-$HOME/.local/bin/python3.11}
T=$HOME/.agent-desktop-bench-target/heldout-tools
fail=0
check() { if eval "$2" > /dev/null 2>&1; then echo "ok    $1"; else echo "FAIL  $1"; fail=1; fi; }
check "tasks_heldout.py matches the frozen sha256" \
  "[ \"\$(shasum -a 256 $H/tasks_heldout.py | cut -d' ' -f1)\" = \"\$(grep -oE 'tasks_heldout.py sha256 \`[0-9a-f]{64}\`' $H/TASKS_FROZEN.md | grep -oE '[0-9a-f]{64}')\" ]"
check "upstream agent-desktop binary" "[ -x $HOME/.t3/worktrees/agent-desktop/bench-upstream/target/release/agent-desktop ]"
check "agent-desktop 1.4.0 checksum" "(cd $T/downloads/agent-desktop-1.4.0 && grep aarch64-apple-darwin.tar.gz checksums.txt | grep -v ffi | shasum -a 256 -c)"
check "peekaboo 4.9.8 checksum" "(cd $T/downloads/peekaboo-4.9.8 && grep 'peekaboo-macos-arm64.tar.gz' checksums.txt | shasum -a 256 -c)"
check "cua-driver 0.46.1 checksum" "(cd $T/downloads/cua-driver-0.46.1 && grep 'darwin-arm64.tar.gz\$' checksums.txt | shasum -a 256 -c)"
check "playwright-cli 0.1.24 installed" "grep -q '\"version\": \"0.1.24\"' $T/playwright-cli-0.1.24/node_modules/@playwright/cli/package.json"
check "browser-harness 0.1.13 in bh-venv" "ls $HOME/.agent-desktop-bench-target/bh-venv/lib/python*/site-packages/browser_harness-0.1.13.dist-info"
check "skills for peekaboo, cua-driver, playwright-cli" "[ -s $T/skills/peekaboo/SKILL.md ] && [ -s $T/skills/cua-driver/SKILL.md ] && [ -s $T/skills/playwright-cli/SKILL.md ]"
check "verifier helper builds" "swiftc -O $H/ho_probe.swift -o $H/out/ho_probe"
check "unit tests pass" "$P -I -m unittest discover -s $H/tests -p 'test_*.py'"
check "Google Chrome is not running" "! pgrep -x 'Google Chrome'"
check "TextEdit is not running" "! pgrep -x TextEdit"
check "Preview is not running" "! pgrep -x Preview"
echo "--- permission status (read-only; the docs of both tools say these never prompt)"
"$T/peekaboo-4.9.8/peekaboo-macos-arm64/peekaboo" permissions status --all-sources --json 2>&1 | head -40
CUA_DRIVER_RS_TELEMETRY_ENABLED=0 CUA_TELEMETRY_ENABLED=false CUA_DRIVER_RS_HOME="$T/cua-home" \
  "$T/cua-driver-0.46.1/cua-driver-rs-0.46.1-darwin-arm64/cua-driver" permissions status --json 2>&1 | head -40
exit $fail
