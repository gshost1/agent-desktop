### Documentation

* The Linux skill, its references and the Linux and configuration docs pages now say a state root that group or other users can reach fails `PERM_DENIED` with `details.kind: "state_directory_permission"` and the `chmod` that closes it, as the code has answered since #41, instead of `INTERNAL`; a skill test fails if any skill pairs that refusal with `INTERNAL` again. ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
