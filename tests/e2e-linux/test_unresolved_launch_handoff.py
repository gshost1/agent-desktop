import copy
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from runner.legs import leg_by_id
from scenarios.launch import unresolved_handoff_pid
from scenarios import launch


class UnresolvedHandoffTests(unittest.TestCase):
    def test_cleanup_failure_does_not_mask_the_body_failure(self):
        ctx = Mock()
        ctx.tier.runtime_dir = Path("/private")
        owner = SimpleNamespace(pid=9999999)
        with patch.object(launch, "members", return_value=[]), \
                patch.object(launch.apps, "owners_named", side_effect=[[], [owner]]), \
                patch.object(launch, "unresolved_handoff_pid", side_effect=AssertionError("body failure")), \
                patch.object(launch, "changed", side_effect=lambda read, expected, **kw: expected):
            ctx.product.return_value = SimpleNamespace(ok=False, raw="cleanup failure")
            with self.assertRaisesRegex(AssertionError, "body failure"):
                launch.unresolved_terminal_handoff(ctx)
        ctx.finding.assert_called_once_with("Terminal cleanup: cleanup failure")

    def test_successful_body_reports_cleanup_failure(self):
        ctx = Mock()
        ctx.tier.runtime_dir = Path("/private")
        ctx.tier.env = {"PATH": "/bin"}
        owner = SimpleNamespace(pid=9999999, start_time=1, windows=["w"])
        ok = SimpleNamespace(ok=True, data={"apps": [{"pid": owner.pid, "name": "Terminal"}]})
        ctx.product.side_effect = [ok, ok, SimpleNamespace(ok=False, raw="cleanup failure")]
        with patch.object(launch, "members", return_value=[]), \
                patch.object(launch.apps, "owners_named", side_effect=[[], [owner], [owner]]), \
                patch.object(launch, "unresolved_handoff_pid", return_value=42), \
                patch.object(launch.raw, "poll", return_value=[owner]), \
                patch.object(launch, "which", return_value="/launcher"), \
                patch.object(launch, "facts", return_value=(0, 0, "", {})), \
                patch.object(launch, "changed", side_effect=lambda read, expected, **kw: expected):
            with self.assertRaisesRegex(AssertionError, "Terminal cleanup: cleanup failure"):
                launch.unresolved_terminal_handoff(ctx)

    def test_cleanup_collects_owner_and_restore_failures_without_stopping_early(self):
        ctx = Mock()
        ctx.product.return_value = SimpleNamespace(ok=False, raw="close failed")
        with patch.object(launch, "changed", side_effect=[RuntimeError("native failed"), []]):
            problems = launch._cleanup_handoff(ctx, lambda: [SimpleNamespace(pid=1)],
                                               [("atspi", lambda: (), ()), ("proc", lambda: [], [])])
        self.assertEqual(problems, ["close failed", "native failed"])
        ctx.restored.assert_called_once_with("proc", [])

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
