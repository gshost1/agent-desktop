"""Reference values for heldout/stats.py, checked against published tables and the plan's power figures."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import stats  # noqa: E402


class WilsonTests(unittest.TestCase):
    def test_reference_values(self):
        lo, hi = stats.wilson(48, 50)
        self.assertAlmostEqual(lo, 0.8654, places=3)
        self.assertAlmostEqual(hi, 0.9890, places=3)
        lo, hi = stats.wilson(0, 10)
        self.assertEqual(lo, 0.0)
        self.assertAlmostEqual(hi, 0.2775, places=3)

    def test_all_passed_stays_below_one_from_the_low_side(self):
        lo, hi = stats.wilson(14, 14)
        self.assertLess(lo, 0.9)
        self.assertAlmostEqual(hi, 1.0)
        self.assertAlmostEqual(lo, 0.7847, places=3)


class FisherTests(unittest.TestCase):
    def test_plan_power_figures(self):
        self.assertAlmostEqual(stats.fisher(48, 50, 38, 50), 0.0076, places=3)
        self.assertGreater(stats.fisher(48, 50, 43, 50), 0.1)

    def test_tea_tasting(self):
        self.assertAlmostEqual(stats.fisher(3, 4, 1, 4), 0.4857, places=3)

    def test_equal_rates(self):
        self.assertAlmostEqual(stats.fisher(5, 10, 5, 10), 1.0)


class SignTests(unittest.TestCase):
    def test_reference(self):
        self.assertAlmostEqual(stats.sign_test(9, 1), 0.0215, places=3)
        self.assertAlmostEqual(stats.sign_test(10, 0), 0.00195, places=4)
        self.assertEqual(stats.sign_test(0, 0), 1.0)
        self.assertAlmostEqual(stats.sign_test(5, 5), 1.0)


if __name__ == "__main__":
    unittest.main()
