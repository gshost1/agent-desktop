import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from scenarios.ref_contract import captured_twin_refused, row_verdict


class CapturedTwinRefusalTests(unittest.TestCase):
    def setUp(self):
        self.error = {"code": "AMBIGUOUS_TARGET",
                      "details": {"kind": "identity_not_unique", "retryable": False},
                      "disposition": {"delivery": "not_delivered", "retry": "safe"}}

    def test_captured_twin_refusal_is_specific_and_safe(self):
        self.assertTrue(captured_twin_refused(self.error))
        for key, value in (("code", "ACTION_FAILED"), ("details", {}),
                           ("details", {"kind": "live_identity_ambiguous", "retryable": False}),
                           ("details", {"kind": "identity_not_unique", "retryable": True}),
                           ("disposition", {"delivery": "delivered_unverified", "retry": "unsafe"})):
            with self.subTest(key=key, value=value):
                changed = copy.deepcopy(self.error)
                changed[key] = value
                self.assertFalse(captured_twin_refused(changed))

    def test_missing_error_is_not_a_refusal(self):
        self.assertFalse(captured_twin_refused(None))
        self.assertFalse(captured_twin_refused({}))

    def test_only_expected_refusals_with_unchanged_independent_oracles_pass(self):
        args = dict(succeeded=False, refused=captured_twin_refused(self.error),
                    log_effect=False, raw_effect=False, log_unchanged=True, raw_unchanged=True)
        self.assertEqual(row_verdict("refuse", **args), "pass")
        self.assertEqual(row_verdict("land", **args), "fail")
        for oracle in ("log_unchanged", "raw_unchanged"):
            self.assertEqual(row_verdict("refuse", **{**args, oracle: False}), "fail")
