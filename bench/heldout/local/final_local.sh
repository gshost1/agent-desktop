#!/bin/bash
# The final arms pass on this Mac, in the logged-in user's GUI session, with no ssh. Same pass as the bench Mac's
# (remote/arms_pass.sh: pinned set and candidates, manifest, saved schedule, per-run reserve against the cap, audit,
# front sampling, ledger), started detached with a clean environment.
#
#   bash local/final_local.sh preflight                       privacy preflight only
#   bash local/final_local.sh start <name> <arms,csv> <taskset>   detached; prints the log path
#   bash local/final_local.sh status <name>                   the log's last lines
#   bash local/final_local.sh stop                            the pass ends after its current run
#
# Everything lives under HO_LOCAL_ROOT (~/.agent-desktop-bench-target/local-final): bench-tools/logs, the STOP
# files, the model's working folders (bench-tools/work, outside any repository), TMPDIR and an empty ZDOTDIR. The
# pass gets only HOME, USER, LOGNAME, SHELL, LANG and the bench variables: no key, token or agent-session variable
# of the calling shell. local/claude.zsh is the pass's `claude`: bare mode on the bench API key, read from the
# owner-only file <root>/secrets/anthropic_api_key into the model process's environment only.
#
# The pass refuses to start (exit 4) while the privacy preflight fails: a mail, messages, photos or notes app, a web
# browser, or a Finder window. It never quits an app. HO_PREFLIGHT_ALLOW (bundle ids) and the file
# <root>/preflight-allow (one bundle id per line) name https-registered apps that are not browsers.
#
# Env: CAP_USD (35), MODEL (claude-sonnet-5-5), RULES (v2). A dry run sets HO_MODEL_STUB_CONFIG=<config.json>:
# local/model_stub.py answers every model call on a local port (no spend; HO_STUB_WITH_KEY=1 sends the bench key to
# it, to check the pass's auth), and HO_PREFLIGHT=record lets a dry run go
# on with a failing preflight, recording it. A real pass cannot skip the preflight.
set -u
H=$(cd "$(dirname "$0")/.." && pwd)
LB=${HO_LOCAL_ROOT:-$HOME/.agent-desktop-bench-target/local-final}
BT=$LB/bench-tools
CLAUDE_SHA=abc9feeeca8091d51b623291e044cf0528c2f6b74e0c1d6ef671d52c73f7cd3c
CLAUDE_BIN=$HOME/.agent-desktop-bench-target/remote/stage/claude-code-2.1.321-bin/claude
PY=$(realpath "$(command -v "${HELDOUT_PY:-python3.11}")")
mkdir -p "$BT/logs" "$BT/tools" "$BT/tmp" "$BT/work" "$BT/zdot" "$BT/claude-bin"

allow() {
  local ids=${HO_PREFLIGHT_ALLOW:-}
  [ -f "$LB/preflight-allow" ] && ids="$ids,$(grep -v '^#' "$LB/preflight-allow" | tr '\n' ',')"
  echo "${ids#,}"
}
preflight() {
  local bin=$H/out/privacy_preflight
  if [ ! -x "$bin" ] || [ "$H/local/privacy_preflight.swift" -nt "$bin" ]; then
    swiftc -O "$H/local/privacy_preflight.swift" -o "$bin" || return 2
  fi
  HO_PREFLIGHT_ALLOW=$(allow) "$bin"
}

case "${1:-}" in
  preflight) preflight; exit $? ;;
  status) tail -n 30 "$BT/logs/arms-$2.log"; exit 0 ;;
  stop) touch "$BT/STOP" && echo "STOP written: the pass ends after its current run"; exit 0 ;;
  start) ;;
  *) sed -n 2,24p "$0"; exit 2 ;;
esac
NAME=$2; ARMS_CSV=$3; TASKSET=$4
[[ $NAME =~ ^[a-z0-9][a-z0-9.-]{0,40}$ ]] || { echo "bad pass name" >&2; exit 2; }
[ "$(shasum -a 256 "$CLAUDE_BIN" | cut -d' ' -f1)" = "$CLAUDE_SHA" ] || { echo "pinned claude binary missing or changed" >&2; exit 3; }
if pgrep -f "remote/arms_pass.sh" > /dev/null; then echo "a pass is already running" >&2; exit 3; fi
rm -f "$BT/STOP" "$BT/STOP-SPEND" "$BT/STOP-API"
cp "$H/local/api_refusal.py" "$BT/tools/api_refusal.py"
ln -sfn "$H/local/claude.zsh" "$BT/claude-bin/claude"
chmod 755 "$H/local/claude.zsh"
LOG=$BT/logs/arms-$NAME.log
pf=$(preflight); rc=$?
echo "[$(date +%H:%M:%S)] PREFLIGHT $pf" >> "$LOG"
if [ $rc -ne 0 ]; then
  if [ -n "${HO_MODEL_STUB_CONFIG:-}" ] && [ "${HO_PREFLIGHT:-}" = record ]; then
    echo "[$(date +%H:%M:%S)] PREFLIGHT failed; dry run goes on (HO_PREFLIGHT=record)" >> "$LOG"
  else
    echo "privacy preflight failed: $pf" >&2; exit 4
  fi
fi
KEY=$LB/secrets/anthropic_api_key
if [ -z "${HO_MODEL_STUB_CONFIG:-}" ] || [ "${HO_STUB_WITH_KEY:-}" = 1 ]; then
  [ -s "$KEY" ] && [ "$(stat -f %Lp "$KEY")" = 600 ] || { echo "no owner-only API key file at $KEY" >&2; exit 3; }
fi
STUB=()
if [ -n "${HO_MODEL_STUB_CONFIG:-}" ]; then
  STUB=(HO_MODEL_STUB_CONFIG="$HO_MODEL_STUB_CONFIG" HO_MODEL_STUB_PORT=18790 HO_STUB_WITH_KEY="${HO_STUB_WITH_KEY:-}")
fi
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" SHELL=/bin/zsh LANG=en_US.UTF-8 TMPDIR="$BT/tmp/" ZDOTDIR="$BT/zdot" \
  PATH="$BT/claude-bin:/usr/bin:/bin:/usr/sbin:/sbin" PYTHONDONTWRITEBYTECODE=1 BENCH_REAL_HOME="$LB" HELDOUT_PY="$PY" \
  HELDOUT_HOST=control-mac HO_WORK_ROOT="$BT/work" CAP_USD="${CAP_USD:-35}" HO_API_KEY_FILE="$KEY" ${STUB[@]+"${STUB[@]}"} \
  /usr/bin/perl -MPOSIX -e 'fork and exit 0; POSIX::setsid(); open STDIN, "<", "/dev/null"; my $l = shift @ARGV; open STDOUT, ">>", $l; open STDERR, ">&STDOUT"; exec @ARGV or die' \
  "$LOG" /bin/bash "$H/local/final_pass.sh" "$NAME" "$ARMS_CSV" "$TASKSET" "${MODEL:-claude-sonnet-5-5}" "${RULES:-v2}"
echo "started $NAME; log $LOG"
