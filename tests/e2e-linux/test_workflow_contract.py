import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent


class WorkflowTests(unittest.TestCase):
    def test_settings_mouse_fixture_tool_is_provisioned(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        install = workflow.split("sudo apt-get install -y --no-install-recommends", 1)[1]
        packages = install.split("\n          if ", 1)[0].replace("\\", "").split()
        self.assertIn("umockdev", packages)

    def test_settings_panels_have_their_video_plugin_and_mutter_schemas(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        install = workflow.split("sudo apt-get install -y --no-install-recommends", 1)[1]
        packages = install.split("\n          if ", 1)[0].replace("\\", "").split()
        for package in ("gstreamer1.0-plugins-base", "mutter-common", "gnome-shell-common", "gnome-shell"):
            self.assertIn(package, packages)

    def test_private_session_failure_logs_are_uploaded(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        checks = workflow.split("- name: Private session checks", 1)[1]
        self.assertIn("if: ${{ !cancelled() }}", checks.split("run:", 1)[0])
        self.assertIn("AGENT_DESKTOP_SESSION_LOG_DIR: ${{ runner.temp }}/private-session-logs", checks)
        upload = checks.split("- name: Upload the private session logs", 1)[1]
        self.assertIn("if: ${{ failure() && steps.private-session.outcome == 'failure' }}", upload)
        self.assertIn("uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a", upload)
        self.assertIn("path: ${{ runner.temp }}/private-session-logs/", upload)
        self.assertIn("retention-days: 7", upload)

    def test_live_workflow_fails_closed(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        self.assertIn("workflow_dispatch", workflow)
        self.assertIn("LINUX_E2E_RUNNER", workflow)
        self.assertIn("tests/e2e-linux/run.sh", workflow)
        job, steps = workflow.split("\n    steps:\n", 1)
        self.assertNotIn("continue-on-error", steps)
        self.assertEqual(job.count(
            "\n    continue-on-error: ${{ github.event_name == 'pull_request' "
            "|| !vars.LINUX_E2E_RUNNER }}\n"), 1, job)
        self.assertNotIn("continue-on-error: true", workflow)

    def test_pull_requests_never_reach_the_self_hosted_runner(self):
        workflow = (REPO / ".github" / "workflows" / "linux-e2e.yml").read_text()
        self.assertIn(
            "runs-on: ${{ (github.event_name != 'pull_request' && vars.LINUX_E2E_RUNNER) "
            "|| 'ubuntu-24.04' }}", workflow)
        uses = [line for line in workflow.splitlines()
                if "vars.LINUX_E2E_RUNNER" in line and "runs-on" not in line]
        self.assertTrue(uses)
        for line in uses:
            self.assertRegex(line, r"github\.event_name [!=]= 'pull_request'", line)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", workflow)

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
