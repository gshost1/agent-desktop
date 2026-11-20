import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from scenarios.ref_contract import unsupported, row_verdict


class CapturedTwinRefusalTests(unittest.TestCase):
    def setUp(self):
        self.error = {"code": "AMBIGUOUS_TARGET",
                      "details": {"kind": "identity_not_unique", "retryable": False},
                      "disposition": {"delivery": "not_delivered", "retry": "safe"}}

    def test_a_twin_row_does_not_pass_the_unsupported_action_contract(self):
        self.assertFalse(unsupported(self.error))
        args = dict(succeeded=False, refused=unsupported(self.error),
                    log_effect=False, raw_effect=False, log_unchanged=True, raw_unchanged=True)
        for expected in ("refuse", "land", "land-or-refuse"):
            self.assertEqual(row_verdict(expected, **args), "fail")

    def test_missing_error_is_not_a_refusal(self):
        self.assertFalse(unsupported(None))
        self.assertFalse(unsupported({}))

    def test_only_expected_refusals_with_unchanged_independent_oracles_pass(self):
        error = {"code": "ACTION_NOT_SUPPORTED", "details": {"semantic_verb": "absent"},
                 "disposition": {"delivery": "not_delivered", "retry": "safe"}}
        args = dict(succeeded=False, refused=unsupported(error),
                    log_effect=False, raw_effect=False, log_unchanged=True, raw_unchanged=True)
        self.assertEqual(row_verdict("refuse", **args), "pass")
        self.assertEqual(row_verdict("land", **args), "fail")
        for oracle in ("log_unchanged", "raw_unchanged"):
            self.assertEqual(row_verdict("refuse", **{**args, oracle: False}), "fail")
