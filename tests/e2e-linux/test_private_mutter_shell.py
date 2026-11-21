import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts/private-mutter-shell.sh"


class PrivateMutterShellTests(unittest.TestCase):
    def run_wrapper(self, args, private=True):
        with tempfile.TemporaryDirectory() as root:
            env = {**os.environ, "PATH": root}
            env.pop("AGENT_DESKTOP_PRIVATE_SESSION", None)
            if private:
                env["AGENT_DESKTOP_PRIVATE_SESSION"] = root
            return subprocess.run(
                ["/bin/bash", str(SCRIPT), *args], env=env,
                capture_output=True, text=True, timeout=5,
            )

    def test_missing_gnome_shell_fails_before_starting_session(self):
        result = self.run_wrapper(["--", "/bin/true"])
        self.assertEqual(result.returncode, 1)
        self.assertIn("prerequisite missing: gnome-shell on PATH", result.stderr)
        self.assertIn("install the gnome-shell package", result.stderr)
        self.assertNotIn("no healthy gnome-shell", result.stderr)

    def test_empty_command_reports_usage(self):
        result = self.run_wrapper([])
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage:", result.stderr)

    def test_requires_private_session(self):
        result = self.run_wrapper(["/bin/true"], private=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn("run it through scripts/private-session.sh", result.stderr)


@unittest.skipUnless(all(shutil.which(tool) for tool in (
    "Xvfb", "dbus-daemon", "dbus-update-activation-environment", "busctl", "flock"
)), "private session tools are required")
class PrivateSessionLogTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.logs = self.root / "logs"
        self.base = self.root / "sessions"
        self.env = {
            **os.environ,
            "AGENT_DESKTOP_SESSION_BASE": str(self.base),
            "AGENT_DESKTOP_SESSION_LOG_DIR": str(self.logs),
        }

    def run_session(self, command):
        return subprocess.run(
            ["/bin/bash", str(REPO / "scripts/private-session.sh"), "--", *command],
            env=self.env, capture_output=True, text=True, timeout=30,
        )

    def test_dead_shell_preserves_every_attempt_and_prints_last_log(self):
        binary = self.root / "gnome-shell"
        binary.write_text("#!/bin/sh\necho intentional-shell-failure >&2\nexit 42\n")
        binary.chmod(0o700)
        self.env["PATH"] = f"{self.root}:{os.environ['PATH']}"
        result = self.run_session(["/bin/bash", str(SCRIPT), "--", "/bin/true"])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("last attempt log follows\nintentional-shell-failure", result.stderr)
        attempts = list(self.logs.glob("*/gnome-shell-mutter-attempt-*.log"))
        self.assertEqual(len(attempts), 5)
        self.assertTrue(all("intentional-shell-failure" in path.read_text() for path in attempts))
        latest = list(self.logs.glob("*/gnome-shell-mutter.log"))
        self.assertEqual(len(latest), 1)
        self.assertFalse(latest[0].is_symlink())
        self.assertEqual(list(self.base.glob("rt.*")), [])

    def test_success_preserves_logs_but_not_private_state(self):
        result = self.run_session([
            "/bin/sh", "-c",
            'echo payload-log > "$XDG_RUNTIME_DIR/payload.log"; '
            'echo private-state > "$XDG_RUNTIME_DIR/private-state"',
        ])
        self.assertEqual(result.returncode, 0, result.stderr)
        payload = list(self.logs.glob("*/payload.log"))
        self.assertEqual(len(payload), 1)
        self.assertEqual(payload[0].read_text(), "payload-log\n")
        self.assertFalse(list(self.logs.glob("*/private-state")))
        self.assertEqual(list(self.base.glob("rt.*")), [])

    def test_unwritable_log_destination_keeps_payload_status_and_cleanup(self):
        self.logs.write_text("not a directory")
        result = self.run_session(["/bin/sh", "-c", "exit 41"])
        self.assertEqual(result.returncode, 41, result.stderr)
        self.assertIn("could not preserve logs", result.stderr)
        self.assertEqual(list(self.base.glob("rt.*")), [])
