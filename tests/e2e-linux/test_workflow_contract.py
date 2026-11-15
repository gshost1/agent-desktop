import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent


class WorkflowTests(unittest.TestCase):
    def test_live_workflow_fails_closed(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        self.assertIn("workflow_dispatch", workflow)
        self.assertIn("LINUX_E2E_RUNNER", workflow)
        self.assertIn("tests/e2e-linux/run.sh", workflow)
        job, steps = workflow.split("\n    steps:\n", 1)
        self.assertNotIn("continue-on-error", steps)
        self.assertEqual(job.count("\n    continue-on-error: true\n"), 1, job)

    def test_hosted_lane_runs_the_self_tests(self):
        ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
        discover = [line for line in ci.splitlines() if "discover -s tests/e2e-linux" in line]
        self.assertEqual(len(discover), 1, discover)
        self.assertIn("AGENT_DESKTOP_E2E_SKIP_LIVE=1", discover[0])

    def test_hosted_lane_tests_run_on_the_bus_they_start(self):
        ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
        steps = ci.split("\n      - name: ")
        runs = [step for step in steps
                if "dbus-run-session" in step and "cargo-test-isolated-home.sh test" in step]
        self.assertEqual(len(runs), 1, runs)
        self.assertIn("AGENT_DESKTOP_TEST_SESSION: inherit", runs[0])
        self.assertIn('AGENT_DESKTOP_EXPECT_SESSION_BUS: "1"', runs[0])
        self.assertIn('AGENT_DESKTOP_EXPECT_ATSPI_BUS: "1"', runs[0])

    def test_live_workflow_runs_on_pull_requests_and_main(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        triggers = workflow.split("\npermissions:", 1)[0]
        self.assertIn("\n  pull_request:\n", triggers)
        self.assertIn("\n  push:\n    branches: [main]", triggers)
        self.assertIn("--tier x11-openbox", workflow)

    def test_live_workflow_writes_the_harness_summary_even_when_legs_fail(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        steps = workflow.split("\n      - name: ")
        summary = [step for step in steps if "runner/step_summary.py" in step]
        self.assertEqual(len(summary), 1, summary)
        self.assertIn("always()", summary[0])
        self.assertIn('>> "$GITHUB_STEP_SUMMARY"', summary[0])
        self.assertIn("/ad-sessions/e2e-suite-*/report.txt", summary[0])
        harness = [step for step in steps if "--tier x11-openbox" in step]
        self.assertEqual(len(harness), 1, harness)
        self.assertIn("AGENT_DESKTOP_SESSION_BASE: ${{ runner.temp }}/ad-sessions", harness[0])
        self.assertLess(steps.index(harness[0]), steps.index(summary[0]))



class SourceRuleTests(unittest.TestCase):
    def test_harness_cleanup_does_not_depend_on_a_trash_command(self):
        names = ("private-session.sh", "private-wayland-shell.sh",
                 "test-private-session.sh", "check-elf-glibc-floor-selftest.sh",
                 "ci-ffi-helper-discovery-smoke.sh")
        trash = re.compile(r"\btrash\b")
        paths = [REPO / "scripts" / name for name in names]
        paths += sorted((ROOT / "runner").glob("*.py")) + sorted((ROOT / "scenarios").glob("*.py"))
        paths += sorted((REPO / "tests" / "linux-dist").glob("run-*.sh"))
        paths = [path for path in paths if path.exists()]
        self.assertEqual([str(path) for path in paths if trash.search(path.read_text())], [])

    def test_runner_files_stay_under_the_line_cap(self):
        over = [
            path.name
            for path in (ROOT / "runner").glob("*.py")
            if len(path.read_text().splitlines()) >= 400
        ]
        self.assertEqual(over, [])
