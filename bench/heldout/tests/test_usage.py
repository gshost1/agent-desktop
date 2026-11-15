"""Token, turn and price accounting of claude -p stream-json runs (bench/driver.py, bench/pricing.py, report.py)."""
import sys
import tempfile
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HELDOUT))
sys.path.insert(0, str(HELDOUT.parent))
import driver  # noqa: E402
import pricing  # noqa: E402
import report  # noqa: E402

MODEL = "claude-sonnet-5-5"


def assistant(mid, out, t, inp=100, read=1000):
    return {"type": "assistant", "_t": t, "message": {"id": mid, "content": [], "usage": {
        "input_tokens": inp, "cache_read_input_tokens": read, "cache_creation_input_tokens": 0, "output_tokens": out}}}


def stream(with_result):
    """Two API calls whose stream usage shows 3 and 5 output tokens; the result event says 900."""
    evs = [{"type": "system", "subtype": "init", "model": MODEL, "_t": 0.0},
           assistant("m1", 3, 1.0), assistant("m2", 5, 2.0)]
    if with_result:
        evs.append({"type": "result", "_t": 3.0, "result": "DONE", "num_turns": 2, "total_cost_usd": 0.0135,
                    "usage": {"input_tokens": 200, "cache_read_input_tokens": 2000,
                              "cache_creation_input_tokens": 0, "output_tokens": 900}})
    return evs


class DriverUsageTests(unittest.TestCase):
    def summarize(self, with_result):
        with tempfile.TemporaryDirectory() as tmp:
            return driver.summarize(stream(with_result), Path(tmp), 3.0)

    def test_output_tokens_come_from_the_result_usage(self):
        s = self.summarize(True)
        self.assertEqual(s["output_tokens"], 900)
        self.assertTrue(s["usage_complete"])
        self.assertEqual(s["cost_usd_list"], 0.0135)
        self.assertFalse(s["cost_estimated"])

    def test_capped_run_keeps_a_lower_bound_and_an_estimated_price(self):
        s = self.summarize(False)
        self.assertEqual(s["output_tokens"], 8)
        self.assertFalse(s["usage_complete"])
        self.assertIsNone(s["num_turns"])
        self.assertTrue(s["cost_estimated"])
        self.assertAlmostEqual(s["cost_usd_list"], (200 * 2.0 + 2000 * 0.1 + 8 * 10.0) / 1e6)


class PricingTests(unittest.TestCase):
    def test_rate_reproduces_a_reported_price(self):
        usage = {"input_tokens": 61, "cache_read_input_tokens": 154_210, "cache_creation_input_tokens": 9_884,
                 "output_tokens": 1_316}
        self.assertAlmostEqual(pricing.list_cost(MODEL, usage), (61 * 2 + 154_210 * 0.1 + 9_884 * 2.5 + 1_316 * 10) / 1e6)

    def test_unknown_model_has_no_price(self):
        self.assertIsNone(pricing.list_cost("other-model", {"input_tokens": 5}))
        self.assertEqual(pricing.row_cost({"model": "other-model", "result_usage": {"input_tokens": 5}}), (None, True))

    def test_stored_timeout_row_is_priced_not_free(self):
        row = {"model": MODEL, "cost_usd_list": None, "num_turns": None, "api_calls": 67, "output_tokens": 400,
               "result_usage": {"input_tokens": 0, "cache_read_input_tokens": 2_000_000,
                                "cache_creation_input_tokens": 0, "output_tokens": 400}}
        cost, estimated = pricing.row_cost(row)
        self.assertTrue(estimated)
        self.assertAlmostEqual(cost, 0.204)
        self.assertEqual(pricing.row_turns(row), 67)

    def test_stored_row_tokens_use_the_final_output(self):
        row = {"suite": "native", "arm": "a", "task": "t", "success": True, "input_total": 1000, "output_tokens": 7,
               "num_turns": 4, "result_usage": {"output_tokens": 120}}
        n = report.normalize(row)
        self.assertEqual(n["tokens"], 1120)
        self.assertEqual(n["turns"], 4)


if __name__ == "__main__":
    unittest.main()
