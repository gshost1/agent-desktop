import inspect
import sys
import tempfile
import unittest
from contextlib import nullcontext
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from runner import apps, legs, main, readiness, report, session


class WarmupTests(unittest.TestCase):
    def test_warmup_has_its_own_mapping_budget(self):
        tier = SimpleNamespace(display=":150", runtime_dir=Path("/private"))
        with patch.object(apps, "launch", return_value=object()) as launch, \
             patch.object(apps, "stop"), patch.object(apps, "tool"), \
             patch.object(apps.proc, "members", return_value=[]), \
             patch.object(apps.time, "sleep"), \
             patch.object(apps.time, "monotonic", side_effect=[0, 0, 2]):
            apps.prime_desktop(tier)
        self.assertEqual(launch.call_args_list, [
            call(tier, name, settle_s=45.0, stderr_log=Path(f"/private/warmup-{index}.stderr.log"))
            for index, name in enumerate(("Calculator", "Files", "Terminal", "Text Editor", "Settings"))
        ])
        self.assertEqual(inspect.signature(apps.launch).parameters["settle_s"].default, 5.0)
        self.assertEqual(main.TIER_DEADLINE_S, 90.0)

    def test_warmup_app_stderr_tail_reaches_printed_tier_failure(self):
        payload = b"DISCARDED PREFIX\n" + b"x" * 4096 + b"\xff\nmock startup failure\n"

        def spawn(*args, stdout, stderr, **kwargs):
            stdout.write(b"NOT STDERR\n")
            stderr.write(payload)
            return SimpleNamespace(pid=999)

        with tempfile.TemporaryDirectory() as directory:
            tier = SimpleNamespace(display=None, runtime_dir=Path(directory), env={})
            (tier.runtime_dir / "app-launch.log").write_text("previous app error\n")
            leg = legs.leg_by_id("observation.qualified_refs_survive_snapshots")
            with patch.object(session, "start_tier", return_value=nullcontext(tier)), \
                 patch.object(readiness, "wait_ready"), patch.object(apps, "_owners", return_value=[]), \
                 patch.object(apps.subprocess, "Popen", side_effect=spawn), \
                 patch.object(apps.raw, "poll", side_effect=TimeoutError("mapping expired")), \
                 patch.dict(apps._SPAWNED, clear=True):
                outcomes = main._run_tier("x11-openbox", [leg], {}, Path(directory), False)
            printed = report.render(outcomes)
        self.assertEqual([row.outcome for row in outcomes], [report.TIER_UNAVAILABLE])
        self.assertIn("desktop warm-up failed: Calculator: mapping expired", printed)
        self.assertIn("stderr tail (last 2048 bytes): " + payload[-2048:].decode("utf-8", errors="replace").strip(), printed)
        for excluded in ("DISCARDED PREFIX", "previous app error", "NOT STDERR"):
            self.assertNotIn(excluded, printed)

    def test_warmup_stderr_tail_handles_empty_missing_and_unreadable_files(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stderr.log"
            self.assertIn("stderr unavailable", apps._warmup_stderr_tail(path))
            path.touch()
            self.assertEqual(apps._warmup_stderr_tail(path), "<empty stderr>")
            with patch.object(Path, "open", side_effect=PermissionError("cannot read stderr")):
                self.assertIn("cannot read stderr", apps._warmup_stderr_tail(path))


if __name__ == "__main__":
    unittest.main()
