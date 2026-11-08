"""Run with: python3 -I -m unittest discover -s bench/browser -p 'test_*.py'"""
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
        patches = [mock.patch.object(run, "launch_chrome", fake_launch),
                   mock.patch.object(run, "stop_chrome", self.stopped.append)]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        self.addCleanup(run.shutil.rmtree, self.work, True)

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


if __name__ == "__main__":
    unittest.main()
