#!/bin/zsh
# Runs the real tool, prints its stdout, and appends one line per call to the run's tool log:
#   t0<TAB>t1<TAB>rc<TAB>stdout_bytes<TAB>args   (the format bench/driver.py summarize() reads)
# The log is $AD_LOG (native runs) or <run work dir>/tool.log (web runs, from AGENT_DESKTOP_HOME).
# Stdout goes to a temp file, not a pipe, so a daemon the tool forks cannot hold the call open.
zmodload zsh/datetime
real=$1; shift
log=${AD_LOG:-}
[[ -z $log && -n ${AGENT_DESKTOP_HOME:-} ]] && log=${AGENT_DESKTOP_HOME:h}/tool.log
tmp=$(mktemp "${TMPDIR:-/tmp}/ho-out.XXXXXX") || exit 70
t0=$EPOCHREALTIME
"$real" "$@" > "$tmp"
rc=$?
t1=$EPOCHREALTIME
cat "$tmp"
n=$(stat -f %z "$tmp")
rm -f "$tmp"
[[ -n $log ]] && print -r -- "$t0	$t1	$rc	$n	${(q)@}" >> "$log"
exit $rc
