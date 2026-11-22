from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from scenarios import launch


class LaunchRecoveryTests(unittest.TestCase):
    def run_leg(self, recovery):
        error = {
            "code": "STALE_REF",
            "details": {"process_state": "exited", "retryable": False},
            "disposition": {"delivery": "not_delivered", "retry": "safe"},
            "recovery": recovery,
        }
        ctx = Mock()
        ctx.product.side_effect = [
            SimpleNamespace(ok=True, raw="snapshot", data={"tree": {
                "role": "button", "name": "7", "ref_id": "@s:e1",
            }}),
            SimpleNamespace(ok=False, raw="stale-ref", error=error),
        ]
        with patch.object(launch, "launch", return_value=SimpleNamespace(pid=42)) as opened, \
                patch.object(launch, "stop") as stopped, \
                patch.object(launch, "members", return_value=[]):
            launch.recovery_strategy_on_error(ctx)
            opened.assert_called_once()
            stopped.assert_called_once()
        self.assertEqual(ctx.product.call_count, 2)
        self.assertEqual(ctx.product.call_args.args[0], ["click", "@s:e1"])
        ctx.restored.assert_called_once_with("proc.calculator", False)

    def test_dead_owner_requires_inspect_state_and_a_fresh_snapshot(self):
        self.run_leg({
            "strategy": "inspect_state_then_retry_original",
            "requires_fresh_snapshot": True,
        })

    def test_missing_or_incorrect_recovery_is_rejected(self):
        for recovery in [
            None,
            {},
            {"strategy": "refresh_snapshot_then_retry_original", "requires_fresh_snapshot": True},
            {"strategy": "inspect_state_then_retry_original"},
            {"strategy": "inspect_state_then_retry_original", "requires_fresh_snapshot": False},
        ]:
            with self.subTest(recovery=recovery), self.assertRaises(AssertionError):
                self.run_leg(recovery)


if __name__ == "__main__":
    unittest.main()
