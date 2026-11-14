#!/bin/zsh
# cand_start.sh <log name> <cand_pass.sh args...>: starts a candidate pass detached, inside the bench
# account's GUI session (through benchrun, which holds the Accessibility and Screen Recording grants).
# Run over ssh with the real HOME; env.sh switches HOME to the bench home.
source $HOME/bench-tools/env.sh
export LANG=en_US.UTF-8
unset LC_ALL LC_CTYPE
H=$HOME/.t3/worktrees/agent-desktop/perf-speed-cost/bench/heldout
cd ${H:h}
log=$BENCH_REAL_HOME/bench-tools/logs/$1; shift
exec $BENCH_REAL_HOME/bench-tools/tools/br /usr/bin/perl -MPOSIX -e 'fork and exit 0; POSIX::setsid(); open STDIN, "<", "/dev/null"; my $l = shift @ARGV; open STDOUT, ">>", $l; open STDERR, ">&STDOUT"; exec @ARGV or die' \
  "$log" /bin/bash $H/remote/cand_pass.sh "$@"
