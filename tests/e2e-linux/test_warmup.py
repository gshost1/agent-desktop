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
            for index, name in enumerate(("Calculator", "Files", "Terminal", "Text Editor", "Settings Mouse"))
        ])
        self.assertEqual(inspect.signature(apps.launch).parameters["settle_s"].default, 5.0)
        self.assertEqual(main.TIER_DEADLINE_S, 90.0)

    def test_settings_warmup_uses_the_mock_mouse_and_captures_wrapper_stderr(self):
        launch = apps.launch
        for fails in (False, True):
            with self.subTest(fails=fails), tempfile.TemporaryDirectory() as directory:
                tier = SimpleNamespace(display=":150", name="x11-openbox", runtime_dir=Path(directory), env={})
                owner = apps.OwnedApp("gnome-control-center", 42, 10, [":1.42"], ["w-1.42-frame"], ":150")
                clock = [0.0]

                def sleep(seconds):
                    clock[0] += seconds

                def warmup(tier, name, **kwargs):
                    return launch(tier, name, **kwargs) if name == "Settings Mouse" else object()

                def spawn(argv, *, stdout, stderr, **kwargs):
                    self.assertEqual(argv[:2], ["umockdev-run", "--device"])
                    self.assertTrue(Path(argv[2]).is_file())
                    self.assertEqual(argv[3:], ["--", "gnome-control-center"])
                    self.assertEqual(Path(stderr.name), tier.runtime_dir / "warmup-4.stderr.log")
                    self.assertNotEqual(stdout, stderr)
                    stderr.write(b"wrapped Settings diagnostic\n")
                    if fails:
                        raise OSError("wrapped launch failed")
                    return SimpleNamespace(pid=999)

                with patch.object(apps, "launch", side_effect=warmup), \
                     patch.object(apps, "which", return_value="/usr/bin/umockdev-run"), \
                     patch.object(apps, "stop"), patch.object(apps, "tool"), \
                     patch.object(apps, "_owners", side_effect=[[], [owner]]), \
                     patch.object(apps, "_display_verified", return_value=True), \
                     patch.object(readiness, "wait_ready"), \
                     patch.object(apps.raw, "poll", side_effect=lambda predicate, *_a: predicate()), \
                     patch.object(apps.subprocess, "Popen", side_effect=spawn) as popen, \
                     patch.object(apps.proc, "members", return_value=[]), \
                     patch.object(apps.time, "sleep", side_effect=sleep), \
                     patch.object(apps.time, "monotonic", side_effect=lambda: clock[0]), \
                     patch.dict(apps._SPAWNED, clear=True), patch.dict(apps._TIERS, clear=True):
                    if fails:
                        with self.assertRaisesRegex(RuntimeError, "Settings Mouse: wrapped launch failed.*wrapped Settings diagnostic"):
                            apps.prime_desktop(tier)
                    else:
                        apps.prime_desktop(tier)
                    popen.assert_called_once()
                self.assertEqual((tier.runtime_dir / "warmup-4.stderr.log").read_text(), "wrapped Settings diagnostic\n")

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
