"""Run with python3 scripts/ci_macos_scope_test.py."""

import contextlib
import importlib.util
import io
import os
import subprocess
import tempfile
import unittest
from unittest import mock
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("ci_macos_scope", SCRIPTS / "ci_macos_scope.py")
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


class MacosScopeTests(unittest.TestCase):
    def test_push_and_dispatch_always_run_macos(self):
        self.assertTrue(SCOPE.event_needs_macos("push", "false", ["docs/a.md"]))
        self.assertTrue(SCOPE.event_needs_macos("workflow_dispatch", "", None))

    def test_a_draft_pull_request_never_runs_macos(self):
        self.assertFalse(SCOPE.event_needs_macos("pull_request", "true", ["crates/macos/src/lib.rs"]))

    def test_an_unreadable_diff_runs_macos(self):
        self.assertTrue(SCOPE.event_needs_macos("pull_request", "false", None))
        self.assertTrue(SCOPE.event_needs_macos("pull_request", "false", []))

    def test_linux_windows_and_docs_changes_skip_macos(self):
        paths = [
            "crates/linux/src/actions/select/select_combo_row.rs",
            "crates/windows/src/adapter.rs",
            "tests/e2e-linux/scenarios/select.py",
            "skills/agent-desktop-linux/SKILL.md",
            "apps/docs/content/index.mdx",
            "changelog.d/fix-linux-combo.md",
            "README.md",
            ".github/workflows/linux-e2e.yml",
            "scripts/linux-live-gate.py",
            "scripts/run-windows-e2e-ci.ps1",
            "tests/e2e-linux/scenarios/helper.sh",
        ]
        self.assertFalse(SCOPE.event_needs_macos("pull_request", "false", paths))

    def test_any_shared_or_macos_path_runs_macos(self):
        for shared in (
            "crates/core/src/commands/click.rs",
            "crates/macos/src/adapter.rs",
            "crates/ffi/src/lib.rs",
            "crates/web/src/lib.rs",
            "src/main.rs",
            "Cargo.lock",
            "skills/agent-desktop/SKILL.md",
            "skills/agent-desktop-macos/SKILL.md",
            "tests/e2e/run.sh",
            ".github/workflows/ci.yml",
            "scripts/ci_macos_scope.py",
            "scripts/check-bash3-compat.sh",
            "npm/package.json",
            "scripts/private-mutter-shell.sh",
            "tests/e2e-linux/run.sh",
            ".githooks/pre-commit",
        ):
            with self.subTest(path=shared):
                self.assertTrue(
                    SCOPE.event_needs_macos("pull_request", "false", ["docs/a.md", shared])
                )

    def test_a_rename_out_of_a_macos_path_runs_macos(self):
        with tempfile.TemporaryDirectory() as directory:
            def git(*args):
                subprocess.run(["git", "-C", directory, *args], check=True, capture_output=True)
            git("init", "-q")
            git("config", "user.email", "scope@example.invalid")
            git("config", "user.name", "scope")
            source = Path(directory, "crates/macos/src/moved.rs")
            source.parent.mkdir(parents=True)
            source.write_text("pub fn moved() {}\n" * 20)
            git("add", "-A")
            git("commit", "-q", "-m", "base")
            Path(directory, "crates/linux/src").mkdir(parents=True)
            git("mv", "crates/macos/src/moved.rs", "crates/linux/src/moved.rs")
            git("commit", "-q", "-m", "move")
            previous = Path.cwd()
            os.chdir(directory)
            try:
                changed = SCOPE.read_merge_commit_changes()
            finally:
                os.chdir(previous)
            self.assertIn("crates/macos/src/moved.rs", changed)
            self.assertTrue(SCOPE.event_needs_macos("pull_request", "false", changed))

    def run_main(self, argv, git_result):
        output = io.StringIO()
        with mock.patch.object(SCOPE.subprocess, "run", return_value=git_result) as run, \
                contextlib.redirect_stdout(output), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(SCOPE.main(argv), 0)
        return output.getvalue(), run

    def test_main_runs_macos_when_the_merge_commit_diff_fails(self):
        failed = subprocess.CompletedProcess([], 128, "", "fatal: bad revision HEAD^1")
        output, _ = self.run_main(["--event", "pull_request", "--draft", "false"], failed)
        self.assertEqual(output, "macos=true\n")

    def test_main_reads_the_merge_commit_diff_of_a_ready_pull_request(self):
        docs_only = subprocess.CompletedProcess([], 0, "docs/a.md\ncrates/linux/src/lib.rs\n", "")
        output, run = self.run_main(["--event", "pull_request", "--draft", "false"], docs_only)
        self.assertEqual(output, "macos=false\n")
        self.assertEqual(run.call_args.args[0], ["git", "diff", "--no-renames", "--name-only", "HEAD^1", "HEAD"])

    def test_main_skips_the_diff_for_a_draft(self):
        output, run = self.run_main(["--event", "pull_request", "--draft", "true"], None)
        self.assertEqual(output, "macos=false\n")
        run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
