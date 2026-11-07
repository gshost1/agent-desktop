# Writer brief (state-machine fix, one PR's worth of work)

You own ONE git worktree and ONE branch, named in your task. Work only inside that worktree and only on the files
your task lists (plus new files you create next to them). Do not touch other worktrees, do not push, do not open PRs,
do not run `git checkout` of other branches. The parent session reviews, pushes and opens the PR.

Repo rules (read AGENTS.md in your worktree; they bind every change): app-agnostic fixes; at most 400 lines per
hand-written .rs file; NO inline comments (`//` lines are rejected; `///` on public items and `//!` module docs only);
one domain struct/enum per file; at most 7 fields per struct and 5 params per fn; no unwrap/expect/panic outside
tests; tests sit beside code as `foo_tests.rs` declared with `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;`;
core never imports platform crates; Conventional Commits, message says why, no AI attribution or co-author lines.

Method:
1. Read the finding in the audit file named in your task (it has file:line pieces, the states, a test sketch).
   Re-verify it against the code before changing anything; if it is not real, stop and report why.
2. Make the state explicit: an enum or typestate with one transition function, illegal states unrepresentable where
   Rust allows. Keep the change as small as that allows; split pure refactor from behavior change only if natural.
3. Write the test that fails on the bug FIRST, run it on the unfixed code and capture the failure, then fix.
4. Invert-verify: break the fix (restore the old code), confirm the test fails, restore the fix.
5. Run the gates and paste the result: `bash audit/tools/gates.sh <your worktree>` (fmt, clippy -D warnings, source-rule and
   architecture scripts, Windows cross-check of core, cargo tree, full host-set tests under a throwaway Xvfb).
   If a test fails only because a socket path is too long or a display is missing, say so; do not "fix" it.
6. Commit on your branch with `export PATH=$HOME/.cargo/bin:$PATH` so the pre-commit hook runs. One or two commits.

Cargo: always set `CARGO_TARGET_DIR=<your worktree>/target` (the gate script does this implicitly by cd).
Toolchain: `export PATH=$HOME/.cargo/bin:$PATH`. Host set: -p agent-desktop-core -p agent-desktop-linux -p agent-desktop -p agent-desktop-ffi.

Report back (max 40 lines): what you changed (files), the state diagram before/after in text, the failing-test output
on unfixed code (verbatim key lines), the invert-verify result, the gate result line, any perf impact, risks, and
anything you found but did not fix. If you could not finish, say exactly where you stopped.
