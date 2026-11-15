### Testing

* The FFI test `session_adapter_keeps_exact_ref_missing_generation_fail_closed` holds its own isolated `HOME`: it read a `HOME` that a parallel test deletes when it ends, so session adapter creation failed with `HOME directory not found` about once in 100,000 creations under four concurrent `HOME` swaps, and once on Windows CI. ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
