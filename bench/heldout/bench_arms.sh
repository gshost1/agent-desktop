#!/bin/bash
# An interleaved multi-arm native pass on the remote bench Mac, from the control Mac, in one command.
#
#   TASKSET=<module> bash bench/heldout/bench_arms.sh <name> <arms, comma-separated> [task ids|all] [n]
#   bash bench/heldout/bench_arms.sh --status <name>       tail the remote log of the pass
#   TASKSET=<module> bash bench/heldout/bench_arms.sh --collect <name>   fetch its rows, report against the first arm
#   bash bench/heldout/bench_arms.sh --stop                ask the running pass to stop after its current run
#
# Arms are built-in arms (ad-14, ad-up, peekaboo, cua) or candidates. CANDIDATES=<label>=<path>,... copies those
# agent-desktop builds (helper beside each) to the bench Mac first, refusing a label that already holds another
# binary; a candidate registered earlier needs no entry. remote/arms_pass.sh refuses the pass unless the task
# set's FROZEN.md pins every file and every candidate arm's binary and helper ("arm `<label>` agent-desktop sha256
# `<hex>`"). Round r rotates the tasks by r and, per task t, the arms by r + t; each task block holds the desktop
# lock once. The first arm is the report's focus.
#
# It refuses when the estimate passes CAP_USD (default 15, list price; the remote pass also stops itself past it),
# or when any pass is already running. The estimate is n x tasks x the sum of per-run prices: ARM_USD=<arm>=<usd>,...
# overrides the defaults below (the stored tuned-set means; 0.07 for a candidate).
# Env: TASKSET (required), MODEL (claude-sonnet-5-5), CAP_USD, ARM_USD, CANDIDATES, RULES (v2), BENCH_HOST,
# BENCH_USER_FILE.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
BENCH_HOST=${BENCH_HOST:-100.111.59.3}
BENCH_USER_FILE=${BENCH_USER_FILE:-$HOME/.t3/userdata/secrets/files/cf0799bbd72ebce8960876e9f8890bbf}
SSHDIR=$HOME/.agent-desktop-bench-target/remote/ssh
RESULTS=$HOME/.agent-desktop-bench-target/remote/results
MODEL=${MODEL:-claude-sonnet-5-5}
CAP_USD=${CAP_USD:-15}
RULES=${RULES:-v2}
PY=${HELDOUT_PY:-python3.11}
BUILTIN=" ad-14 ad-up peekaboo cua "
DEFAULT_USD="ad-14=0.107,ad-up=0.091,peekaboo=0.078,cua=0.077"
CANDIDATE_USD=0.07

U=$(<"$BENCH_USER_FILE")
SSHOPTS=(-o SetEnv=LANG=en_US.UTF-8 -o BatchMode=yes -o ConnectTimeout=20 -i "$SSHDIR/bench_ed25519" -o "UserKnownHostsFile=$SSHDIR/known_hosts")
mask() { sed "s/$U/<user>/g"; }
r() { ssh "${SSHOPTS[@]}" "$U@$BENCH_HOST" "$@" 2> >(mask >&2) | mask; }
put() { rsync -a -e "ssh ${SSHOPTS[*]}" "$@" 2>&1 | mask; }
BH="bench-tools/home"
MIRROR="$BH/.t3/worktrees/agent-desktop/perf-speed-cost/bench"
LOGS="bench-tools/logs"
CANDS="$BH/.agent-desktop-bench-target/heldout-tools/candidates"

check_name() { [[ $1 =~ ^[a-z0-9][a-z0-9.-]{0,40}$ ]] || { echo "name must match [a-z0-9][a-z0-9.-]*: $1" >&2; exit 2; }; }
need_taskset() {
  [ -n "${TASKSET:-}" ] && [ -f "$TASKSET" ] && [ -f "$(dirname "$TASKSET")/FROZEN.md" ] \
    || { echo "TASKSET must name a task module with a FROZEN.md beside it" >&2; exit 2; }
}
taskset_info() {
  "$PY" -I - "$TASKSET" "$@" <<'PY'
import importlib.util, sys
spec = importlib.util.spec_from_file_location("heldout_taskset", sys.argv[1])
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
if sys.argv[2] == "name":
    print(m.NAME)
else:
    native = list(m.NATIVE_TASKS)
    picked = native if sys.argv[3] == "all" else sys.argv[3].split(",")
    unknown = [t for t in picked if t not in native]
    if unknown:
        sys.exit(f"unknown native task {unknown}")
    print(len(picked))
PY
}

collect() {
  local name=$1 set; set=$(taskset_info name)
  local dir=$RESULTS/candidates/$set
  mkdir -p "$dir/raw-arms-$name"
  put "$U@$BENCH_HOST:$MIRROR/heldout/runs/$set/results-native.jsonl" "$dir/raw-arms-$name/" || { echo "no rows on the bench Mac" >&2; exit 1; }
  put "$U@$BENCH_HOST:$MIRROR/heldout/runs/$set/attempts.jsonl" "$dir/raw-arms-$name/" 2>/dev/null || true
  put "$U@$BENCH_HOST:$LOGS/manifest-$name.json" "$dir/raw-arms-$name/" 2>/dev/null || true
  local spec; spec=$(r "cat $LOGS/arms-$name.start")
  "$PY" -I - "$dir/raw-arms-$name/results-native.jsonl" "$spec" "$dir/arms-$name-results-native.jsonl" "$HERE/.." <<'PY'
import json, sys
from pathlib import Path
src, spec, out = Path(sys.argv[1]), sys.argv[2].split(), Path(sys.argv[3])
sys.path.insert(0, sys.argv[4])
import pricing
arms, start = spec[1].split(","), int(spec[2])
every = [json.loads(l) for l in src.read_text().splitlines() if l.strip()]
tagged = [r for r in every if r.get("experiment") == spec[0]]
rows = tagged if tagged else every[start:]
rows = [r for r in rows if r.get("arm") in arms]
out.write_text("".join(json.dumps(r) + "\n" for r in rows))
for arm in arms:
    mine = [r for r in rows if r["arm"] == arm]
    print(f"{arm}: {len(mine)} rows, list price ${sum(pricing.row_cost(r)[0] or 0.0 for r in mine):.2f}")
print(f"total list price ${sum(pricing.row_cost(r)[0] or 0.0 for r in rows):.2f}")
PY
  local focus; focus=$(r "cut -d' ' -f2 $LOGS/arms-$name.start" | cut -d, -f1)
  "$PY" -I "$HERE/report.py" --model "$MODEL" --focus "$focus" --results "$dir/arms-$name-results-native.jsonl" > "$dir/REPORT-arms-$name.md"
  local sched; sched=$(dirname "$TASKSET")/schedule.json
  if [ -f "$sched" ] && grep -q "\"experiment\": \"$name\"" "$sched"; then
    "$PY" -I "$HERE/final_report.py" --schedule "$sched" --ledger "$dir/raw-arms-$name/attempts.jsonl" \
      --rows "$dir/arms-$name-results-native.jsonl" --focus "$focus" > "$dir/FINAL-arms-$name.md"
    echo "final report: $dir/FINAL-arms-$name.md"
  fi
  echo "report: $dir/REPORT-arms-$name.md"
}

case "${1:-}" in
  --status) check_name "$2"; r "tail -n 25 $LOGS/arms-$2.log"; exit 0 ;;
  --collect) check_name "$2"; need_taskset; collect "$2"; exit 0 ;;
  --stop) r "touch bench-tools/STOP && echo STOP written"; exit 0 ;;
  ""|-h|--help) sed -n 2,22p "$0"; exit 2 ;;
esac

NAME=$1; ARMS_CSV=$2; TASKS=${3:-all}; N=${4:-5}
check_name "$NAME"; need_taskset
[[ $N =~ ^[1-9][0-9]?$ ]] || { echo "n must be 1-99" >&2; exit 2; }
[[ $RULES =~ ^v[12]$ ]] || { echo "RULES must be v1 or v2" >&2; exit 2; }
nt=$(taskset_info count "$TASKS") || exit 2
IFS=, read -r -a ARMS <<< "$ARMS_CSV"
CAND_LABELS=(); CAND_BINS=()
if [ -n "${CANDIDATES:-}" ]; then
  IFS=, read -r -a entries <<< "$CANDIDATES"
  for e in "${entries[@]}"; do CAND_LABELS+=("${e%%=*}"); CAND_BINS+=("${e#*=}"); done
fi
for arm in "${ARMS[@]}"; do
  check_name "$arm"
  if [[ $BUILTIN != *" $arm "* ]] && [[ " ${CAND_LABELS[*]:-} " != *" $arm "* ]]; then
    r "test -x $CANDS/$arm/agent-desktop" || { echo "arm $arm is neither built in, in CANDIDATES, nor registered on the bench Mac" >&2; exit 2; }
  fi
done
est=$("$PY" -I - "$N" "$nt" "$DEFAULT_USD,${ARM_USD:-}" "$CANDIDATE_USD" "${ARMS[@]}" <<'PY'
import sys
n, nt, table, cand = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3], float(sys.argv[4])
price = dict(e.split("=") for e in table.split(",") if "=" in e)
per = {a: float(price.get(a, cand)) for a in sys.argv[5:]}
for a, p in per.items():
    print(f"  {a}: {n} x {nt} x ${p} = ${n * nt * p:.2f}", file=sys.stderr)
print(round(n * nt * sum(per.values()), 2))
PY
)
echo "estimate: \$$est (cap \$$CAP_USD), $((N * nt * ${#ARMS[@]})) runs"
"$PY" -I -c "import sys; sys.exit(0 if $est <= $CAP_USD else 1)" || { echo "estimate passes CAP_USD; not starting" >&2; exit 4; }

busy=$(r "pgrep -fl 'cand_pass.sh|arms_pass.sh|remote_run_all.sh|ab_pass.sh|web_pass.sh' | grep -v pgrep | wc -l | tr -d ' '")
[ "$busy" = 0 ] || { echo "a pass is already running on the bench Mac; not starting" >&2; exit 5; }
for (( c=0; c<${#CAND_LABELS[@]}; c++ )); do
  arm=${CAND_LABELS[$c]}; bin=${CAND_BINS[$c]}; check_name "$arm"; helper=$(dirname "$bin")/agent-desktop-macos-helper
  [ -f "$bin" ] && [ -f "$helper" ] || { echo "candidate $arm needs $bin and its helper" >&2; exit 2; }
  sha=$(shasum -a 256 "$bin" | cut -c1-16)
  old=$(r "cat $CANDS/$arm/VERSION 2>/dev/null || true")
  if [ -n "$old" ] && [[ $old != *"sha256 $sha"* ]]; then echo "label $arm already holds another binary ($old)" >&2; exit 6; fi
  r "mkdir -p $CANDS/$arm"
  put "$bin" "$helper" "$U@$BENCH_HOST:$CANDS/$arm/"
  printf '%s\n' "$("$bin" --version) candidate $arm sha256 $sha" > "${TMPDIR:-/tmp}/ho-arms-version"
  put "${TMPDIR:-/tmp}/ho-arms-version" "$U@$BENCH_HOST:$CANDS/$arm/VERSION"
done
r "rm -f bench-tools/STOP bench-tools/STOP-SPEND"
put --exclude 'runs*' --exclude 'out/' --exclude '__pycache__' --include 'arms/front/*.swift' --exclude '*.swift' --exclude 'REPORT*' \
  --exclude 'COSTS*' "$HERE/" "$U@$BENCH_HOST:$MIRROR/heldout/"
put "$HERE/../driver.py" "$HERE/../pricing.py" "$HERE/../tasks.py" "$HERE/../lockq.sh" "$U@$BENCH_HOST:$MIRROR/"
TS_DIR=$(cd "$(dirname "$TASKSET")" && pwd); TS_REMOTE=.agent-desktop-bench-target/$(basename "$TS_DIR")
r "mkdir -p -m 700 $BH/$TS_REMOTE && chmod 700 $BH/$TS_REMOTE"
put --exclude 'feas/' --exclude '__pycache__' --exclude 'tests/' --exclude '*.swift' "$TS_DIR/" "$U@$BENCH_HOST:$BH/$TS_REMOTE/"
r "CAP_USD=$CAP_USD zsh $MIRROR/heldout/remote/arms_start.sh arms-$NAME.log $NAME $ARMS_CSV $TASKS $N $MODEL $TS_REMOTE/$(basename "$TASKSET") $RULES"
echo "started: $NAME, arms $ARMS_CSV, tasks $TASKS, n=$N. Watch: bash $0 --status $NAME; collect: TASKSET=$TASKSET bash $0 --collect $NAME"
