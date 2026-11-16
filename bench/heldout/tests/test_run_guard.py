"""The measured side of a native run (arms/run_guard.py and run_native's capture and grant reads). No GUI.

python3 -I tests/test_run_guard.py
"""
import json
import os
import subprocess
import sys
import time
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HELDOUT))
sys.path.insert(0, str(HELDOUT / "arms"))
import run_guard  # noqa: E402


class FakeClock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        return self.t


def watch(fronts, apps=("Chess",), idle=None):
    clock = FakeClock()
    seq = iter(fronts)
    w = run_guard.FrontWatch(apps, reader=lambda: next(seq), clock=clock, idle=idle or (lambda: {}))
    w._t0 = clock()
    w.start_front = w._last = w.label(next(seq))
    return w, clock


class FrontWatchTests(unittest.TestCase):
    def test_a_bench_app_coming_forward_is_recorded_by_name_and_others_are_not(self):
        w, clock = watch(["Finder", "Finder", "Chess", "Chess", "Notes", "Chess"])
        for _ in range(5):
            clock.t += 0.2
            w.sample()
        rec = w.stop()
        self.assertEqual(rec["front_at_start"], "other")
        self.assertEqual([app for _, app in rec["fronted"]], ["Chess", "Chess"])
        self.assertNotIn("Notes", json.dumps(rec))
        self.assertNotIn("Finder", json.dumps(rec))
        self.assertTrue(run_guard.foreground_violation(rec, False).startswith("foreground: Chess"))
        self.assertIsNone(run_guard.foreground_violation(rec, True))

    def test_a_run_that_stays_in_the_background_passes(self):
        w, clock = watch(["Finder"] * 4)
        for _ in range(3):
            w.sample()
        rec = w.stop()
        self.assertEqual(rec["fronted"], [])
        self.assertIsNone(run_guard.foreground_violation(rec, False))

    def test_a_task_app_in_front_at_the_start_is_the_baseline(self):
        w, clock = watch(["Chess", "Chess", "Chess"], apps=("Chess", "ColorSync Utility"))
        w.sample()
        w.sample()
        rec = w.stop()
        self.assertEqual(rec["front_at_start"], "Chess")
        self.assertIsNone(run_guard.foreground_violation(rec, False))
        w, clock = watch(["Chess", "Chess", "ColorSync Utility"], apps=("Chess", "ColorSync Utility"))
        w.sample()
        w.sample()
        self.assertTrue(run_guard.foreground_violation(w.stop(), False).startswith("foreground: ColorSync"))

    def test_the_holder_is_the_baseline_and_a_task_app_after_it_is_seen(self):
        w, clock = watch(["BenchFront", "BenchFront", "Chess"])
        w.sample()
        w.sample()
        rec = w.stop()
        self.assertEqual(rec["front_at_start"], "holder")
        self.assertTrue(run_guard.foreground_violation(rec, False).startswith("foreground: Chess"))

    def test_hold_front_waits_for_the_holder_and_reports_when_it_does_not_come(self):
        fronts = iter(["Finder", "Chess", "BenchFront"])
        opened = []
        got = run_guard.hold_front(reader=lambda: next(fronts), opener=lambda: opened.append(1), sleep=lambda s: None)
        self.assertIsNone(got)
        self.assertEqual(opened, [1])
        got = run_guard.hold_front(reader=lambda: "Chess", opener=lambda: None, seconds=0.3, sleep=lambda s: None)
        self.assertTrue(got.startswith("BenchFront did not take the front"))

    def test_global_input_during_the_run_is_recorded(self):
        w, clock = watch(["Finder"] * 2, idle=lambda: {"key": 0.5, "mouse_move": 9999.0})
        clock.t += 10
        self.assertEqual(w.stop()["global_input"], ["key"])

    def test_the_live_reader_names_an_app_or_returns_none(self):
        name = run_guard.front_app()
        self.assertTrue(name is None or isinstance(name, str))


class ChildTests(unittest.TestCase):
    def test_the_cap_kills_every_descendant(self):
        child = run_guard.Child(["/bin/sh", "-c", "sleep 60 & sleep 60 & echo $! ; wait"], "/", dict(os.environ))
        grandchild = int(child.proc.stdout.readline())
        child.kill_group()
        child.finish()
        time.sleep(0.2)
        alive = subprocess.run(["kill", "-0", str(grandchild)], capture_output=True).returncode == 0
        self.assertFalse(alive)

    def test_stderr_is_drained_while_stdout_streams(self):
        script = "i=0; while [ $i -lt 2000 ]; do echo eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee >&2; i=$((i+1)); done; echo done"
        child = run_guard.Child(["/bin/sh", "-c", script], "/", dict(os.environ))
        out = child.proc.stdout.read()
        err = child.finish()
        self.assertEqual(out.strip(), "done")
        self.assertGreater(len(err), 60000)


class RunNativeReadsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import importlib.util
        spec = importlib.util.spec_from_file_location("rn_probe", HELDOUT / "arms" / "run_native.py")
        cls.rn = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(cls.rn)
        except SystemExit:
            raise unittest.SkipTest("run_native needs its task set")

    def events(self, *texts):
        return [{"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": str(i), "content": t}
                                                         for i, t in enumerate(texts)]}}]

    def test_a_capture_in_tool_output_is_found(self):
        self.assertIsNotNone(self.rn.captured(self.events('{"snapshot_id":"a","has_screenshot": true}')))
        self.assertIsNotNone(self.rn.captured(self.events('{"data":{"screenshot_raw":"/tmp/p.png"}}')))
        self.assertIsNone(self.rn.captured(self.events('{"has_screenshot":false}', '{"ok":true,"command":"snapshot"}')))

    def test_grant_parsing_reads_each_tools_shape(self):
        texts = {'{"data":{"accessibility":{"state":"granted"},"screen_recording":{"state":"denied"}}}': (True, False),
                 '{"accessibility": true, "screen_recording": true}': (True, True),
                 '{"permissions":{"screenRecording":"granted","accessibility":"granted"}}': (True, True)}
        for text, want in texts.items():
            found = {}
            for name, value in self.rn.GRANT.findall(text):
                key = "accessibility" if name.lower() == "accessibility" else "screen_recording"
                found.setdefault(key, value.lower() in ("true", "granted", "authorized", "yes"))
            self.assertEqual((found.get("accessibility"), found.get("screen_recording")), want, text)


if __name__ == "__main__":
    unittest.main()
