"""The saved pass schedule (schedule.py): complete, balanced, reproducible, and resumable from the ledger.

python3 -I tests/test_schedule.py
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HELDOUT))
import schedule  # noqa: E402

ARMS = ["a", "b", "c", "d", "e"]
TASKS = [f"t{i}" for i in range(10)]


class ScheduleTests(unittest.TestCase):
    def test_five_rounds_of_five_arms_are_complete_and_balanced(self):
        plan = schedule.make("x", ARMS, TASKS, 5, 3)
        self.assertEqual(len(plan["cells"]), 250)
        self.assertEqual(schedule.problems(plan), [])

    def test_the_seed_fixes_the_order_and_another_seed_changes_it(self):
        one, two = schedule.make("x", ARMS, TASKS, 5, 3), schedule.make("x", ARMS, TASKS, 5, 3)
        self.assertEqual(one, two)
        self.assertNotEqual(one["cells"], schedule.make("x", ARMS, TASKS, 5, 4)["cells"])

    def test_a_dropped_or_moved_cell_is_caught(self):
        plan = schedule.make("x", ARMS, TASKS, 5, 3)
        dropped = dict(plan, cells=plan["cells"][:-1])
        self.assertTrue(schedule.problems(dropped))
        moved = json.loads(json.dumps(plan))
        moved["cells"][0]["position"], moved["cells"][1]["position"] = 1, 0
        self.assertTrue(schedule.problems(moved))

    def test_pending_never_reruns_an_attempt_even_an_interrupted_one(self):
        with tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False) as fh:
            for rec in ({"experiment": "x", "slot": 0, "state": "started"},
                        {"experiment": "x", "slot": 0, "state": "finished"},
                        {"experiment": "x", "slot": 1, "state": "started"},
                        {"experiment": "y", "slot": 2, "state": "finished"}):
                fh.write(json.dumps(rec) + "\n")
        self.assertEqual(schedule.attempted_slots("x", fh.name), {0, 1})


if __name__ == "__main__":
    unittest.main()
