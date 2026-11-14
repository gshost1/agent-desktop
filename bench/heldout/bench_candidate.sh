#!/bin/bash
# Bench a candidate agent-desktop binary on the remote bench Mac, from the control Mac, in one command.
#
#   bash bench/heldout/bench_candidate.sh <label> <path-to-agent-desktop> [task ids|all] [n]
#   bash bench/heldout/bench_candidate.sh --status <label>      tail the remote log
#   bash bench/heldout/bench_candidate.sh --collect <label>     fetch the rows, print the report against every arm
#   bash bench/heldout/bench_candidate.sh --stop                ask the running pass to stop after its current run
#
# <label> names the arm (lowercase, digits, dots, dashes; not a built-in arm). The binary's
# agent-desktop-macos-helper must sit next to it (a release build of `-p agent-desktop -p agent-desktop-macos
# --bins` leaves both in target/release). Task ids are comma-separated (hn1,hw3) or all; n is the number of
# rounds (default 5), each round running every task once in the same rotation, seeds and caps as the Sonnet pass.
#
# It refuses when the estimated spend passes CAP_USD (default 15, list price), when a pass is already running,
# or when the label already holds a different binary. It syncs the harness (bench/heldout, without runs, out and
# the Swift helpers, whose compiled copies on the remote must not be rebuilt), copies the binary and helper as
# they are (ad-hoc linker signature kept), and starts cand_pass.sh detached in the remote GUI session. The pass
# stops itself past CAP_USD. WAIT=1 makes the command wait for the pass and collect at the end.
# Env: MODEL (claude-sonnet-5-5), CAP_USD, NOTE (one line for VERSION), BENCH_HOST, BENCH_USER_FILE.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
BENCH_HOST=${BENCH_HOST:-100.111.59.3}
BENCH_USER_FILE=${BENCH_USER_FILE:-$HOME/.t3/userdata/secrets/files/2e9f373fbe3227c5e3303f683361bc8d}
SSHDIR=$HOME/.agent-desktop-bench-target/remote/ssh
RESULTS=$HOME/.agent-desktop-bench-target/remote/results
MODEL=${MODEL:-claude-sonnet-5-5}
CAP_USD=${CAP_USD:-15}
PY=${HELDOUT_PY:-python3.11}
BUILTIN=" ad-14 ad-137 ad-up peekaboo cua bh pw ab "
NATIVE_USD=0.10
WEB_USD=0.03

U=$(<"$BENCH_USER_FILE")
SSHOPTS=(-o SetEnv=LANG=en_US.UTF-8 -o BatchMode=yes -o ConnectTimeout=20 -i "$SSHDIR/bench_ed25519" -o "UserKnownHostsFile=$SSHDIR/known_hosts")
mask() { sed "s/$U/<user>/g"; }
r() { ssh "${SSHOPTS[@]}" "$U@$BENCH_HOST" "$@" 2> >(mask >&2) | mask; }
put() { rsync -a -e "ssh ${SSHOPTS[*]}" "$@" 2>&1 | mask; }
BH="bench-tools/home"
MIRROR="$BH/.t3/worktrees/agent-desktop/perf-speed-cost/bench"
LOGS="bench-tools/logs"

check_label() {
  [[ $1 =~ ^[a-z0-9][a-z0-9.-]{0,40}$ ]] || { echo "label must match [a-z0-9][a-z0-9.-]*: $1" >&2; exit 2; }
  [[ $BUILTIN != *" $1 "* ]] || { echo "label $1 is a built-in arm" >&2; exit 2; }
}

collect() {
  local label=$1 dir=$RESULTS/candidates
  mkdir -p "$dir/raw-$label"
  put "$U@$BENCH_HOST:$MIRROR/heldout/runs/results-native.jsonl" "$U@$BENCH_HOST:$MIRROR/heldout/runs/results-web.jsonl" \
    "$dir/raw-$label/"
  put "$U@$BENCH_HOST:$MIRROR/heldout/runs/web/$label-$MODEL/runs.jsonl" "$dir/raw-$label/web-runs.jsonl" || true
  "$PY" -I - "$dir/raw-$label" "$label" "$dir" <<'PY'
import json, sys
from pathlib import Path
raw, label, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
for name in ("results-native.jsonl", "results-web.jsonl"):
    p = raw / name
    rows = [l for l in p.read_text().splitlines() if l.strip() and json.loads(l).get("arm") == label] if p.exists() else []
    (out / f"{label}-{name}").write_text("".join(l + "\n" for l in rows))
    print(f"{label}-{name}: {len(rows)} rows")
cost = sum(float(json.loads(l).get("cost_usd_list") or 0) for l in (out / f"{label}-results-native.jsonl").read_text().splitlines())
web = raw / "web-runs.jsonl"
cost += sum(float(json.loads(l).get("cost_usd_list") or 0) for l in web.read_text().splitlines() if l.strip()) if web.exists() else 0
print(f"{label} list-price spend: ${cost:.2f}")
PY
  "$PY" -I "$HERE/report.py" --model "$MODEL" --focus "$label" --results "$RESULTS/results-native.jsonl" \
    "$RESULTS/results-web.jsonl" "$RESULTS/ad137-results-web.jsonl" "$dir/$label-results-native.jsonl" \
    "$dir/$label-results-web.jsonl" > "$dir/REPORT-$label.md"
  echo "report: $dir/REPORT-$label.md"
}

case "${1:-}" in
  --status) check_label "$2"; r "tail -n 25 $LOGS/cand-$2.log"; exit 0 ;;
  --collect) check_label "$2"; collect "$2"; exit 0 ;;
  --stop) r "touch bench-tools/STOP && echo STOP written"; exit 0 ;;
  ""|-h|--help) sed -n 2,21p "$0"; exit 2 ;;
esac

LABEL=$1; BIN=$2; TASKS=${3:-all}; N=${4:-5}
check_label "$LABEL"
[ -f "$BIN" ] || { echo "no binary: $BIN" >&2; exit 2; }
HELPER=$(dirname "$BIN")/agent-desktop-macos-helper
[ -f "$HELPER" ] || { echo "no agent-desktop-macos-helper next to $BIN" >&2; exit 2; }
file -b "$BIN" | grep -q arm64 || { echo "$BIN is not an arm64 Mach-O" >&2; exit 2; }
[[ $N =~ ^[1-9][0-9]?$ ]] || { echo "n must be 1-99" >&2; exit 2; }
ALL="hn1 hn2 hn3 hn4 hn5 hn6 hn7 hn8 hw1 hw2 hw3 hw4 hw5 hw6 hw7 hp1 hp2 hp3"
if [ "$TASKS" != all ]; then
  for t in ${TASKS//,/ }; do [[ " $ALL " == *" $t "* ]] || { echo "unknown task $t" >&2; exit 2; }; done
fi
list=${TASKS//,/ }; [ "$TASKS" = all ] && list=$ALL
nn=0; nw=0; for t in $list; do [[ $t == hn* ]] && nn=$((nn + 1)) || nw=$((nw + 1)); done
est=$("$PY" -I -c "print(round($N * ($nn * $NATIVE_USD + $nw * $WEB_USD), 2))")
echo "estimate: $N rounds x ($nn native x \$$NATIVE_USD + $nw web x \$$WEB_USD) = \$$est (cap \$$CAP_USD)"
"$PY" -I -c "import sys; sys.exit(0 if $est <= $CAP_USD else 1)" || { echo "estimate passes CAP_USD; not starting" >&2; exit 4; }

SHA=$(shasum -a 256 "$BIN" | cut -c1-16)
VERSION="$("$BIN" --version) candidate $LABEL sha256 $SHA${NOTE:+ ($NOTE)}"
codesign -dv "$BIN" 2>&1 | grep -q "Signature=adhoc" || echo "warning: $BIN is not ad-hoc signed like ad-14" >&2

busy=$(r "pgrep -fl 'cand_pass.sh|remote_run_all.sh|ab_pass.sh|web_pass.sh' | grep -v pgrep | wc -l | tr -d ' '")
[ "$busy" = 0 ] || { echo "a pass is already running on the bench Mac; not starting" >&2; exit 5; }
CAND="$BH/.agent-desktop-bench-target/heldout-tools/candidates/$LABEL"
old=$(r "cat $CAND/VERSION 2>/dev/null || true")
if [ -n "$old" ] && [[ $old != *"sha256 $SHA"* ]]; then
  echo "label $LABEL already holds another binary ($old); pick a new label" >&2; exit 6
fi
r "rm -f bench-tools/STOP bench-tools/STOP-SPEND; mkdir -p $CAND"
put --exclude 'runs*' --exclude 'out/' --exclude '__pycache__' --exclude '*.swift' --exclude 'REPORT*' \
  --exclude 'COSTS*' "$HERE/" "$U@$BENCH_HOST:$MIRROR/heldout/"
put "$BIN" "$HELPER" "$U@$BENCH_HOST:$CAND/"
printf '%s\n' "$VERSION" > "${TMPDIR:-/tmp}/ho-cand-version"
put "${TMPDIR:-/tmp}/ho-cand-version" "$U@$BENCH_HOST:$CAND/VERSION"
r "cd $CAND && LC_ALL=C shasum -a 256 agent-desktop | cut -c1-16 && ./agent-desktop --version"
r "zsh $MIRROR/heldout/remote/cand_start.sh cand-$LABEL.log $LABEL $TASKS $N $MODEL"
echo "started: $LABEL on $TASKS, n=$N, model $MODEL. Watch: bash $0 --status $LABEL; collect: bash $0 --collect $LABEL"
if [ "${WAIT:-0}" = 1 ]; then
  until r "grep -E 'CAND COMPLETE|ending the pass|SPEND CAP|API REFUSED|no candidate|does not match' $LOGS/cand-$LABEL.log" > /dev/null; do
    sleep 120
  done
  r "tail -n 3 $LOGS/cand-$LABEL.log"
  collect "$LABEL"
fi
