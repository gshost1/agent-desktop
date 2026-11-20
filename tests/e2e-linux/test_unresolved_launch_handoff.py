import copy
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))

from runner.legs import leg_by_id
from scenarios.launch import unresolved_handoff_pid


class UnresolvedHandoffTests(unittest.TestCase):
    def setUp(self):
        self.error = {"code": "ACTION_FAILED", "details": {"kind": "exited", "status": 0, "spawned_pid": 42},
                      "disposition": {"delivery": "delivered_unverified", "retry": "unsafe"}}

    def envelope(self, **changes):
        return SimpleNamespace(**{**dict(ok=False, exit_code=1, error=self.error), **changes})

    def test_successful_launcher_exit_reports_unsafe_unproven_handoff(self):
        self.assertEqual(unresolved_handoff_pid(self.envelope()), 42)
        leg = leg_by_id("launch.unresolved_terminal_handoff")
        self.assertEqual(leg.modes, ("headless",))

    def test_missing_error_and_successful_launch_are_not_accepted(self):
        for changes in ({"error": None}, {"error": {}}, {"ok": True}, {"exit_code": 2}):
            with self.subTest(changes=changes), self.assertRaises(AssertionError):
                unresolved_handoff_pid(self.envelope(**changes))

    def test_unrelated_failures_safe_retries_and_missing_pids_are_not_accepted(self):
        for key, value in (("code", "HARNESS_TIMEOUT"), ("details", {}),
                           ("details", {"kind": "exited", "status": 1, "spawned_pid": 42}),
                           ("details", {"kind": "crashed", "status": 0, "spawned_pid": 42}),
                           ("details", {"kind": "exited", "status": 0, "spawned_pid": True}),
                           ("disposition", {"delivery": "not_delivered", "retry": "safe"}),
                           ("recovery", {"strategy": "relaunch"})):
            changed = copy.deepcopy(self.error)
            changed[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(AssertionError):
                unresolved_handoff_pid(self.envelope(error=changed))
