"""remote/arms_check.py: an arms pass runs only on a pinned task set with pinned candidate builds."""
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "remote"))
import arms_check  # noqa: E402

BUILTIN = {"ad-14", "ad-up", "peekaboo", "cua"}


def sha(data):
    return hashlib.sha256(data).hexdigest()


class ArmsCheckTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.ts = root / "set" / "tasks_x.py"
        self.ts.parent.mkdir()
        self.ts.write_text("NAME = 'x'\n")
        self.cands = root / "candidates"
        (self.cands / "ad-15").mkdir(parents=True)
        (self.cands / "ad-15" / "agent-desktop").write_bytes(b"bin")
        (self.cands / "ad-15" / "agent-desktop-macos-helper").write_bytes(b"helper")
        self.pins = {"file": sha(b"NAME = 'x'\n"), "bin": sha(b"bin"), "helper": sha(b"helper")}

    def tearDown(self):
        self.tmp.cleanup()

    def frozen(self, file=None, binary=None, helper=None):
        lines = [f"`tasks_x.py` sha256 `{file or self.pins['file']}`"]
        if binary is not False:
            lines.append(f"arm `ad-15` agent-desktop sha256 `{binary or self.pins['bin']}`")
        if helper is not False:
            lines.append(f"arm `ad-15` agent-desktop-macos-helper sha256 `{helper or self.pins['helper']}`")
        (self.ts.parent / "FROZEN.md").write_text("\n".join(lines) + "\n")

    def check(self, *arms):
        return arms_check.refusals(self.ts, self.cands, BUILTIN, list(arms))

    def test_pinned_set_and_candidate_pass(self):
        self.frozen()
        self.assertEqual(self.check("ad-15", "ad-14", "peekaboo"), [])

    def test_a_rebuilt_candidate_is_refused(self):
        self.frozen()
        (self.cands / "ad-15" / "agent-desktop").write_bytes(b"rebuilt")
        self.assertEqual(self.check("ad-15"), ["candidate ad-15: agent-desktop differs from its pin"])

    def test_an_unpinned_candidate_or_helper_is_refused(self):
        self.frozen(binary=False)
        self.assertEqual(self.check("ad-15"), ["candidate ad-15: agent-desktop is not pinned"])
        self.frozen(helper=False)
        self.assertEqual(self.check("ad-15"), ["candidate ad-15: agent-desktop-macos-helper is not pinned"])

    def test_an_edited_task_set_is_refused(self):
        self.frozen()
        self.ts.write_text("NAME = 'y'\n")
        self.assertEqual(self.check("ad-14"), ["tasks_x.py differs from its pin"])

    def test_unknown_and_repeated_arms(self):
        self.frozen()
        self.assertEqual(self.check("ad-99"), ["unknown arm ad-99"])
        self.assertEqual(self.check("ad-14", "ad-14"), ["an arm is listed twice"])

    def test_a_set_without_its_module_pin_is_refused(self):
        (self.ts.parent / "FROZEN.md").write_text("nothing pinned\n")
        self.assertEqual(self.check("ad-14"), ["tasks_x.py is not pinned"])


if __name__ == "__main__":
    unittest.main()
