"""Run with python3 scripts/ci_macos_scope_test.py."""

import importlib.util
import unittest
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
            "scripts/private-mutter-shell.sh",
            "scripts/run-windows-e2e-ci.ps1",
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
        ):
            with self.subTest(path=shared):
                self.assertTrue(
                    SCOPE.event_needs_macos("pull_request", "false", ["docs/a.md", shared])
                )


if __name__ == "__main__":
    unittest.main()
