"""Run with: python3 -m unittest discover -s bench/browser -p 'test_*.py'"""
import importlib
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import strays  # noqa: E402


class StraysTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="strays-test."))
        self.addCleanup(strays.shutil.rmtree, self.root, True)
        self.work = self.root / "work"
        (self.work / "sub").mkdir(parents=True)

    def test_a_colon_named_save_in_the_work_folder_is_moved(self):
        since = time.time() - 1
        (self.work / "sub" / ":private:tmp:x1.txt").write_text("q")
        (self.work / "x1.txt").write_text("q")
        moved = strays.collect_strays(since, self.work, self.root / "strays")
        self.assertEqual(moved, ["sub/:private:tmp:x1.txt"])
        self.assertTrue((self.root / "strays" / ":private:tmp:x1.txt").exists())
        self.assertTrue((self.work / "x1.txt").exists())

    def test_an_old_colon_file_is_left_alone(self):
        old = self.work / ":old.txt"
        old.write_text("q")
        os.utime(old, (1, 1))
        self.assertEqual(strays.collect_strays(time.time() - 1, self.work, self.root / "strays"), [])

    def test_the_check_never_reads_the_home_folder(self):
        with mock.patch.object(Path, "home", side_effect=AssertionError("home folder read")), \
             mock.patch.object(Path, "expanduser", side_effect=AssertionError("home folder read")), \
             mock.patch("subprocess.run", side_effect=AssertionError("folder listed outside the work folder")):
            fresh = importlib.reload(strays)
            fresh.collect_strays(time.time() - 1, self.work, self.root / "strays")
        importlib.reload(strays)


if __name__ == "__main__":
    unittest.main()
