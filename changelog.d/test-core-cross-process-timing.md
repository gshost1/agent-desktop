### Testing

* The two cross-process core tests (`two_process_transactions_preserve_disjoint_drill_updates` and `gc_retains_idle_cross_process_owner_and_reaps_after_crash`) no longer depend on how fast the scheduler starts their helper processes. Each helper now reports its rendezvous on stdout and waits for the test's release on stdin, in place of a 2 s ready-file deadline and a 5 s lease hold, and a failed run no longer leaves helpers polling forever. Under 64 CPU spinners with the test at `nice 19` (load average about 88), 30 runs of each went from 6 and 5 failures to 0 and 0.
