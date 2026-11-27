"""Run with python3 scripts/ci_macos_scope_test.py."""

import contextlib
import importlib.util
import io
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("ci_macos_scope", SCRIPTS / "ci_macos_scope.py")
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


class MacosScopeTests(unittest.TestCase):
    def test_only_a_manual_dispatch_starts_hosted_macos(self):
        self.assertTrue(SCOPE.event_needs_macos("workflow_dispatch"))

    def test_pull_requests_and_pushes_never_start_hosted_macos(self):
        for event in ("pull_request", "push", "schedule"):
            with self.subTest(event=event):
                self.assertFalse(SCOPE.event_needs_macos(event))

    def run_main(self, argv):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.assertEqual(SCOPE.main(argv), 0)
        return output.getvalue()

    def test_main_prints_the_output_line(self):
        self.assertEqual(self.run_main(["--event", "pull_request", "--draft", "false"]), "macos=false\n")
        self.assertEqual(self.run_main(["--event", "workflow_dispatch", "--draft", ""]), "macos=true\n")


if __name__ == "__main__":
    unittest.main()
