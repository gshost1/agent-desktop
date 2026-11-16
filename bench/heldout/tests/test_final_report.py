"""The preregistered final report (final_report.py) on synthetic schedules, ledgers and rows. No GUI, no model.

python3 -I tests/test_final_report.py
"""
import sys
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HELDOUT))
import final_report as fr  # noqa: E402
import schedule  # noqa: E402

ARMS = ["cand", "base"]
TASKS = [f"t{i}" for i in range(10)]


def run_all(plan, ok=lambda c: True, wall=lambda c: 50.0, skip=()):
    ledger, rows = [], []
    for c in plan["cells"]:
        ledger.append({"experiment": plan["experiment"], "slot": c["slot"], "state": "started"})
        if c["slot"] in skip:
            continue
        ledger.append({"experiment": plan["experiment"], "slot": c["slot"], "state": "finished"})
        rows.append({"experiment": plan["experiment"], "slot": c["slot"], "run_id": f"r{c['slot']}",
                     "arm": c["arm"], "task": c["task"], "round": c["round"], "success": ok(c), "wall_s": wall(c),
                     "cli_s": 1.0, "input_total": 1000, "num_turns": 5, "cost_usd_list": 0.05})
    return ledger, rows


class FinalReportTests(unittest.TestCase):
    def setUp(self):
        self.plan = schedule.make("exp", ARMS, TASKS, 5, 1)

    def test_a_missing_cell_names_no_winner_and_counts_as_a_failure(self):
        ledger, rows = run_all(self.plan, ok=lambda c: c["arm"] == "cand")
        out = fr.render(self.plan, ledger, rows[:-1], "cand")
        self.assertIn("Incomplete exploratory comparison", out)
        cs, _ = fr.cells(self.plan, ledger, rows[:-1])
        self.assertEqual(sum(c["state"] == "no_row" for c in cs), 1)
        ledger2 = [r for r in ledger if not (r["slot"] == 0 and r["state"] == "finished")]
        cs, _ = fr.cells(self.plan, ledger2, [r for r in rows if r["slot"] != 0])
        self.assertEqual([c["state"] for c in cs if c["slot"] == 0], ["interrupted"])
        self.assertFalse([c for c in cs if c["slot"] == 0][0]["ok"])

    def test_a_clear_paired_win_is_a_win_and_a_null_is_no_difference(self):
        ledger, rows = run_all(self.plan, ok=lambda c: c["arm"] == "cand" or c["round"] < 2)
        out = fr.render(self.plan, ledger, rows, "cand")
        self.assertIn("**Against base: wins.**", out)
        ledger, rows = run_all(self.plan, ok=lambda c: (c["round"] + (c["arm"] == "cand")) % 2 == 0)
        out = fr.render(self.plan, ledger, rows, "cand")
        self.assertIn("**Against base: inconclusive.**", out)
        self.assertIn("did not establish a reliability difference", out)
        self.assertNotIn("tie", out.split("## Attempts")[0].lower().replace("noninferiority", ""))

    def test_rows_from_another_experiment_or_repeated_do_not_count(self):
        ledger, rows = run_all(self.plan)
        extra = [dict(rows[0], experiment="old"), dict(rows[1], run_id="dup")]
        _, ignored = fr.cells(self.plan, ledger, rows + extra)
        self.assertEqual(ignored["other experiment"], 1)
        self.assertEqual(ignored["repeated run id"], 1)

    def test_the_penalty_and_its_sensitivities(self):
        ledger, rows = run_all(self.plan, ok=lambda c: not (c["arm"] == "base" and c["task"] == "t0"))
        cs, _ = fr.cells(self.plan, ledger, rows)
        e = fr.efficiency(cs, "base", "wall")
        self.assertAlmostEqual(e["penalized"], 9 * 50 + 2 * 250)
        self.assertAlmostEqual(e["once"], 9 * 50 + 250)
        self.assertAlmostEqual(e["capped"], 9 * 50 + 300)
        self.assertAlmostEqual(e["pooled_per_success"], 50 * 50 / 45)

    def test_conditional_ratio_finds_a_constant_speedup(self):
        ledger, rows = run_all(self.plan, wall=lambda c: 40.0 if c["arm"] == "cand" else 80.0)
        cs, _ = fr.cells(self.plan, ledger, rows)
        r = fr.conditional_ratio(fr.pairs(cs, "cand", "base"), "wall")
        self.assertAlmostEqual(r["ratio"], 0.5)
        self.assertEqual(r["tasks"], 10)

    def test_holm_is_monotone_and_bounded(self):
        self.assertEqual(fr.holm([0.01, 0.04, 0.03]), [0.03, 0.06, 0.06])


if __name__ == "__main__":
    unittest.main()
