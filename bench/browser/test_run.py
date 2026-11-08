"""Run with: python3 -I -m unittest discover -s bench/browser -p 'test_*.py'"""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run  # noqa: E402


def fake_launch(profile, port, downloads, extra):
    (profile / "Default" / "Cache").mkdir(parents=True)
    (profile / "Default" / "Cache" / "blob").write_bytes(b"x" * 1024)
    return object()


class ChromeSessionTest(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="run-test."))
        self.profile = self.work / "profile"
        self.downloads = self.work / "downloads"
        self.downloads.mkdir()
        (self.downloads / "report.csv").write_text("kept")
        self.stopped = []
        self.dialogs = []
        self.exits = True
        patches = [mock.patch.object(run, "launch_chrome", fake_launch),
                   mock.patch.object(run, "stop_chrome", self.stop),
                   mock.patch.object(run, "bench_dialogs", lambda ad: self.dialogs)]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        self.addCleanup(run.shutil.rmtree, self.work, True)

    def stop(self, proc, profile):
        self.stopped.append(profile)
        return self.exits

    def test_a_finished_run_deletes_its_profile_and_keeps_its_downloads(self):
        with run.chrome_session(self.profile, None, self.downloads, []):
            self.assertTrue(self.profile.exists())
        self.assertFalse(self.profile.exists())
        self.assertEqual((self.downloads / "report.csv").read_text(), "kept")
        self.assertEqual(len(self.stopped), 1)

    def test_a_run_that_raises_still_stops_chrome_and_deletes_its_profile(self):
        with self.assertRaises(RuntimeError):
            with run.chrome_session(self.profile, None, self.downloads, []):
                raise RuntimeError("driver crashed")
        self.assertFalse(self.profile.exists())
        self.assertEqual(len(self.stopped), 1)

    def test_a_chrome_that_never_starts_leaves_no_profile(self):
        def failing_launch(profile, port, downloads, extra):
            fake_launch(profile, port, downloads, extra)
            raise RuntimeError("chrome devtools endpoint never came up")

        with mock.patch.object(run, "launch_chrome", failing_launch):
            with self.assertRaises(RuntimeError):
                with run.chrome_session(self.profile, None, self.downloads, []):
                    pass
        self.assertFalse(self.profile.exists())
        self.assertEqual(self.stopped, [])

    def test_a_profile_is_never_deleted_while_chrome_still_runs_on_it(self):
        self.exits = False
        with run.chrome_session(self.profile, None, self.downloads, []):
            pass
        self.assertTrue(self.profile.exists())

    def test_a_dialog_after_launch_fails_the_run_and_closes_chrome(self):
        self.dialogs = ["alert"]
        with self.assertRaises(run.BenchDialog):
            with run.chrome_session(self.profile, None, self.downloads, [], ad="agent-desktop"):
                self.fail("the run must not start with a dialog on screen")
        self.assertEqual(len(self.stopped), 1)
        self.assertFalse(self.profile.exists())

    def test_a_headless_run_does_not_look_for_dialogs(self):
        self.dialogs = ["alert"]
        with run.chrome_session(self.profile, None, self.downloads, ["--headless=new"], ad="agent-desktop"):
            pass
        self.assertFalse(self.profile.exists())


class LaunchChromeTest(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="launch-test."))
        self.addCleanup(run.shutil.rmtree, self.work, True)
        popen = mock.patch.object(run.subprocess, "Popen", return_value=mock.Mock(pid=0))
        popen.start()
        self.addCleanup(popen.stop)
        sleep = mock.patch.object(run.time, "sleep")
        sleep.start()
        self.addCleanup(sleep.stop)

    def test_a_profile_folder_that_already_exists_is_refused(self):
        (self.work / "profile").mkdir()
        with self.assertRaises(FileExistsError):
            run.launch_chrome(self.work / "profile", None, self.work, [])
        run.subprocess.Popen.assert_not_called()

    def test_preferences_are_complete_json_before_chrome_starts(self):
        seen = {}

        def capture(args, **kwargs):
            seen["prefs"] = json.loads((self.work / "profile" / "Default" / "Preferences").read_text())
            seen["args"] = args
            return mock.Mock(pid=0)

        run.subprocess.Popen.side_effect = capture
        run.launch_chrome(self.work / "profile", None, self.work, [])
        self.assertIn("download", seen["prefs"])
        self.assertIn("--no-first-run", seen["args"])
        self.assertIn("--no-default-browser-check", seen["args"])
        self.assertFalse((self.work / "profile" / "Default" / "Preferences.tmp").exists())


class LaunchChromeAdTest(unittest.TestCase):
    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="launch-ad-test."))
        self.addCleanup(run.shutil.rmtree, self.work, True)

    def test_agent_desktop_launches_chrome_on_the_run_profile_with_its_home(self):
        seen = {}

        def fake_run(cmd, **kwargs):
            seen["cmd"], seen["env"] = cmd, kwargs["env"]
            seen["prefs"] = (self.work / "profile" / "Default" / "Preferences").exists()
            return mock.Mock(stdout=json.dumps({"ok": True, "data": {"pid": 4242}}), stderr="")

        with mock.patch.object(run.subprocess, "run", fake_run):
            chrome = run.launch_chrome_ad("ad", self.work / "home", self.work / "profile", self.work / "dl", [])
        self.assertEqual(chrome.pid, 4242)
        self.assertTrue(seen["prefs"])
        self.assertEqual(seen["cmd"][:6], ["ad", "launch", "Google Chrome", "--cdp", "--download-dir", str(self.work / "dl")])
        self.assertIn(f"--arg=--user-data-dir={self.work / 'profile'}", seen["cmd"])
        self.assertIn("--arg=--no-first-run", seen["cmd"])
        self.assertEqual(seen["env"]["AGENT_DESKTOP_HOME"], str(self.work / "home"))

    def test_a_failed_launch_raises_so_the_session_discards_the_profile(self):
        failed = mock.Mock(stdout=json.dumps({"ok": False, "error": {"code": "APP_NOT_FOUND"}}), stderr="")
        with mock.patch.object(run.subprocess, "run", return_value=failed), \
             mock.patch.object(run, "profile_processes", return_value=[]):
            with self.assertRaises(RuntimeError):
                run.launch_chrome_ad("ad", self.work / "home", self.work / "profile", self.work / "dl", [])


class StopChromeTest(unittest.TestCase):
    def test_stop_waits_until_no_helper_runs_on_the_profile(self):
        helpers = [[11, 12], [12], []]
        calls = []

        def processes(profile):
            calls.append(profile)
            return helpers[min(len(calls) - 1, len(helpers) - 1)] if len(calls) > 2 else helpers[0]

        proc = mock.Mock(pid=10)
        proc.poll.return_value = 0
        with mock.patch.object(run, "profile_processes", side_effect=processes), \
             mock.patch.object(run.os, "killpg"), mock.patch.object(run.os, "kill"), \
             mock.patch.object(run.time, "sleep"):
            self.assertTrue(run.stop_chrome(proc, Path("/nonexistent/profile")))
        self.assertGreaterEqual(len(calls), 3)

    def test_stop_reports_failure_while_a_helper_survives(self):
        proc = mock.Mock(pid=10)
        proc.poll.return_value = 0
        clock = iter(range(0, 10_000))
        with mock.patch.object(run, "profile_processes", return_value=[11]), \
             mock.patch.object(run.os, "killpg"), mock.patch.object(run.os, "kill"), \
             mock.patch.object(run.time, "sleep"), mock.patch.object(run.time, "time", lambda: next(clock)):
            self.assertFalse(run.stop_chrome(proc, Path("/nonexistent/profile")))


if __name__ == "__main__":
    unittest.main()


class CdpPortTest(unittest.TestCase):
    def test_every_run_gets_a_port_nothing_listens_on(self):
        port = run.free_port()
        self.assertEqual(run.listener_pids(port), set())

    def test_an_endpoint_answered_by_another_process_is_not_this_runs(self):
        with mock.patch.object(run, "listener_pids", return_value={11}), \
                mock.patch.object(run, "profile_processes", return_value=[22, 23]):
            self.assertFalse(run.endpoint_is_this_runs(9, Path("p")))

    def test_an_endpoint_nobody_listens_on_is_not_this_runs(self):
        with mock.patch.object(run, "listener_pids", return_value=set()), \
                mock.patch.object(run, "profile_processes", return_value=[22]):
            self.assertFalse(run.endpoint_is_this_runs(9, Path("p")))

    def test_an_endpoint_listened_on_only_by_the_runs_processes_is_this_runs(self):
        with mock.patch.object(run, "listener_pids", return_value={22}), \
                mock.patch.object(run, "profile_processes", return_value=[22, 23]):
            self.assertTrue(run.endpoint_is_this_runs(9, Path("p")))


class ProfileProcessesTest(unittest.TestCase):
    def test_the_pattern_is_passed_after_a_double_dash_because_it_starts_with_dashes(self):
        with mock.patch.object(run.subprocess, "run", return_value=mock.Mock(stdout="")) as called:
            run.profile_processes(Path("/p"))
        self.assertEqual(called.call_args.args[0][:3], ["pgrep", "-f", "--"])

    def test_a_running_process_on_the_profile_is_found(self):
        profile = Path(tempfile.mkdtemp(prefix="pp-test."))
        self.addCleanup(run.shutil.rmtree, profile, True)
        child = run.subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)", f"--user-data-dir={profile}"])
        self.addCleanup(child.kill)
        run.time.sleep(0.5)
        self.assertIn(child.pid, run.profile_processes(profile))
