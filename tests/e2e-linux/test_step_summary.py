import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from runner import report, step_summary
from runner.report import LegOutcome


class StepSummaryTests(unittest.TestCase):
    def rendered(self):
        return report.render([
            LegOutcome("ok.leg", "x11-openbox", "headless", report.PASS),
            LegOutcome("refused.leg", "x11-openbox", "headless", report.REFUSED, "refused as asserted"),
            LegOutcome("broken.leg", "x11-openbox", "headless", report.FAIL,
                       "expected ('TIMEOUT',): {\"a\": \"b|c\"}\nsecond line",
                       findings=("name | role\nNone | group | fail",)),
            LegOutcome("slow.leg", "x11-openbox", "headed", report.ERROR,
                       "TimeoutError: oracle responsiveness deadline expired"),
        ])

    def test_lists_each_failing_leg_with_its_counts_and_reason(self):
        text = step_summary.summarize(self.rendered())
        self.assertIn("**asserted-refusal 1, error 1, fail 1, pass 1**", text)
        self.assertIn("| x11-openbox | headless | `broken.leg` | fail | expected ('TIMEOUT',): {\"a\": \"b\\|c\"} |", text)
        self.assertIn("| x11-openbox | headed | `slow.leg` | error | TimeoutError: oracle responsiveness deadline expired |", text)
        self.assertNotIn("ok.leg", text)
        self.assertNotIn("refused.leg", text)
        self.assertNotIn("None | group", text)

    def test_long_reasons_are_cut_to_one_short_cell(self):
        text = step_summary.summarize(report.render([
            LegOutcome("long.leg", "x11-openbox", "headless", report.FAIL, "x" * 1000)]))
        row = [line for line in text.splitlines() if "long.leg" in line][0]
        self.assertLess(len(row), step_summary.REASON_LIMIT + 80)
        self.assertTrue(row.endswith("… |"))

    def test_a_green_report_says_so(self):
        text = step_summary.summarize(report.render([
            LegOutcome("ok.leg", "x11-openbox", "headed", report.PASS)]))
        self.assertIn("**pass 1**", text)
        self.assertIn("Every leg passed.", text)

    def test_a_missing_report_is_named_instead_of_an_empty_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "e2e-suite-*" / "report.txt"
            out = Path(tmp) / "out.md"
            with out.open("w") as stream:
                saved, sys.stdout = sys.stdout, stream
                try:
                    code = step_summary.main([str(missing)])
                finally:
                    sys.stdout = saved
            self.assertEqual(code, 0)
            self.assertIn("No leg results: no report.txt was written", out.read_text())


if __name__ == "__main__":
    unittest.main()
