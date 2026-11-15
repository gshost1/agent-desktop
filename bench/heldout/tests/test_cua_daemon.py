"""The cua arm's daemon start: a granted app daemon passes, anything else is an infrastructure fault. No GUI.

python3 -I tests/test_cua_daemon.py
"""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "arms"))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import cua_daemon  # noqa: E402

GRANTED = {"accessibility": True, "daemon_running": True, "screen_recording": True}
PENDING = {"daemon_running": False, "status": "unknown", "reason": "Cua Driver isn't installed in Applications"}


def done(code=0, out=""):
    return subprocess.CompletedProcess([], code, out, "")


class StartTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.app = self.tmp / "CuaDriver.app"
        self.app.mkdir()
        self.opened = []

    def start(self, status_codes, perms):
        codes = list(status_codes)

        def prober(args):
            if args == ["status"]:
                return done(codes.pop(0) if codes else 1)
            return done(0, json.dumps(perms))
        return cua_daemon.start(self.tmp / "arm-home", "cua-driver", app=self.app,
                                opener=lambda app, home: self.opened.append((app, home)), prober=prober,
                                sleep=lambda s: None)

    def test_a_granted_app_daemon_is_ready(self):
        self.assertIsNone(self.start([1, 0], GRANTED))
        self.assertEqual(self.opened, [(self.app, self.tmp / "arm-home" / "cua-home")])
        self.assertTrue((self.tmp / "arm-home" / "cua-home" / "telemetry").is_dir())

    def test_a_daemon_without_the_grant_is_a_fault(self):
        fault = self.start([0], dict(GRANTED, accessibility=False))
        self.assertIn("lacks its Accessibility grant", fault)
        self.assertIn("lacks", self.start([0], PENDING))

    def test_a_daemon_that_never_starts_is_a_fault(self):
        self.assertEqual(self.start([], GRANTED), "the CuaDriver.app daemon did not start")

    def test_a_missing_app_is_a_fault_and_opens_nothing(self):
        self.app.rmdir()
        self.assertIn("missing", self.start([0], GRANTED))
        self.assertEqual(self.opened, [])


if __name__ == "__main__":
    unittest.main()
