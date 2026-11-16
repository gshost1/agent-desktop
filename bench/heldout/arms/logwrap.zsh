#!/bin/zsh
# Runs the real tool, prints its stdout, and appends one line per call to the run's tool log:
#   t0<TAB>t1<TAB>rc<TAB>stdout_bytes<TAB>args   (the format bench/driver.py summarize() reads)
# The log is $AD_LOG (native runs) or <run work dir>/tool.log (web runs, from AGENT_DESKTOP_HOME).
# Stdout goes to a temp file, not a pipe, so a daemon the tool forks cannot hold the call open.
# With HO_CAPTURE=off (every native run that does not pass --screen-recording) a call that asks for pixels is
# refused before the tool runs, with the switch that turns capture off; run_native also fails a run whose tool
# output shows a capture that got through (a cua run_script, for one).
zmodload zsh/datetime
real=$1; shift
refuse_capture() { print -u2 "bench: screen capture is off in this benchmark; $1"; exit 64; }
if [[ ${HO_CAPTURE:-on} == off ]]; then
  all=" ${(j: :)@} "
  case ${real:t} in
    agent-desktop)
      [[ ${@[(r)screenshot]} == screenshot || $all == *" --screenshot"* ]] && refuse_capture "read the tree with snapshot" ;;
    peekaboo)
      [[ $1 == (image|capture) ]] && refuse_capture "read the tree with see --no-screenshot"
      [[ $1 == see && $all != *" --no-screenshot "* ]] && refuse_capture "pass --no-screenshot to see" ;;
    cua-driver)
      tool=$1; [[ $1 == call ]] && tool=$2
      [[ $tool == (screenshot|zoom|get_window_screenshot) ]] && refuse_capture "read the tree with include_screenshot:false"
      [[ $all =~ '"include_screenshot" *: *true' ]] && refuse_capture "pass include_screenshot:false"
      if [[ $tool == (get_window_state|verify_state) || $all == *'"observe"'* ]] && [[ ! $all =~ '"include_screenshot" *: *false' ]]; then
        refuse_capture "pass include_screenshot:false"
      fi ;;
  esac
fi
log=${AD_LOG:-}
[[ -z $log && -n ${AGENT_DESKTOP_HOME:-} ]] && log=${AGENT_DESKTOP_HOME:h}/tool.log
tmp=$(mktemp "${TMPDIR:-/tmp}/ho-out.XXXXXX") || exit 70
# The tool runs with HOME at the run's throwaway tool home, so it reads and writes no state of the host account.
tool_home=${HO_TOOL_HOME:-${HO_ARM_HOME:+$HO_ARM_HOME/home}}
[[ -n $tool_home ]] && mkdir -p "$tool_home"
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN
t0=$EPOCHREALTIME
HOME=${tool_home:-$HOME} "$real" "$@" > "$tmp"
rc=$?
t1=$EPOCHREALTIME
cat "$tmp"
n=$(stat -f %z "$tmp")
rm -f "$tmp"
[[ -n $log ]] && print -r -- "$t0	$t1	$rc	$n	${(q)@}" >> "$log"
exit $rc
