"""The whole-run native audit (arms/run_audit.py) and the v2 prompt rules, on commands seen in stored runs."""
import sys
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HELDOUT))
sys.path.insert(0, str(HELDOUT / "arms"))
import common  # noqa: E402
import run_audit  # noqa: E402
import tasks_heldout as th  # noqa: E402

SPILL = "/Users/someone/.claude/projects/-Users-someone/0a1b/tool-results/b6xiran1q.txt"


def run(cmds, arm="ad-14", allow_foreground=False):
    audit = run_audit.RunAudit(arm, th.FORBIDDEN, allow_foreground)
    return [audit.check(c) for c in cmds]


class AllowedTests(unittest.TestCase):
    def test_stored_commands_that_read_only_the_tools_output(self):
        cmds = [
            "agent-desktop snapshot --app TextEdit --surface menubar -i --compact > /tmp/m.json; "
            "grep -o '\"name\":\"Show Fonts\"[^}]*}' /tmp/m.json | head",
            "agent-desktop snapshot --app Preview -i --compact --lines | grep -E 'Width|Height' | head -20",
            "R=$(agent-desktop find --app TextEdit --role textarea | jq -r '.data.matches[0].ref_id'); agent-desktop click $R",
            f"grep -o -i -E '.{{150}}menubar.{{200}}' {SPILL} | head -5",
            "cd /Users/someone/.claude/projects/x/y/tool-results/; grep -o -i 'subscript' b6xiran1q.txt",
            "agent-desktop snapshot --app TextEdit --lines 2>/dev/null | sed -n '1,40p' | cut -c1-150",
            "jq -r .data.lines[] <<< \"$OUT\" | grep -c button",
            "for i in 1 2 3; do agent-desktop press down --app Preview >/dev/null; done; agent-desktop get @s1:e2 --property value",
        ]
        self.assertEqual(run(cmds), [None] * len(cmds))

    def test_guide_reads_with_text_tools(self):
        guide = common.NATIVE_GUIDE_DIR["cua"]
        cmds = [f"grep -n -i -A25 'run_script' {guide}/WORKFLOW.md | head -40",
                f"cd {guide}; grep -n -i -A25 preview MACOS.md | head -80"]
        self.assertEqual(run(cmds, "cua"), [None, None])

    def test_foreground_allowed_when_the_task_allows_it(self):
        self.assertIsNone(run(["agent-desktop press cmd+t --app TextEdit --headed"], allow_foreground=True)[0])


class RefusedTests(unittest.TestCase):
    def test_other_shells_and_indirection(self):
        for cmd in ["sh /tmp/u.sh", "bash -c 'agent-desktop snapshot'", "eval \"$X\"", "source ./x"]:
            self.assertTrue(run([cmd])[0].startswith("shell"), cmd)

    def test_reading_system_or_bench_files(self):
        for cmd in ["head -5 /etc/localtime", "grep -c x ~/bench/notes.wflow", "awk '{print}' /tmp/other.txt",
                    "sed -n 1p doc.applescript", "jq . ~/Library/Preferences/x.json", "wc -l < /etc/hosts",
                    "grep x \"$F\""]:
            self.assertIsNotNone(run([cmd])[0], cmd)

    def test_editing_a_file_in_place(self):
        self.assertEqual(run(["sed -i '' s/a/b/ /tmp/m.json"])[0], "sed -i edits a file")

    def test_clipboard(self):
        self.assertEqual(run(["pbpaste"])[0], "pbpaste")

    def test_foreground_flags_per_tool(self):
        self.assertEqual(run(["agent-desktop press cmd+t --app TextEdit --headed"])[0], "foreground --headed")
        self.assertEqual(run(["peekaboo press cmd+up --no-remote --app TextEdit --foreground --json"], "peekaboo")[0],
                         "foreground --foreground")
        self.assertTrue(run(["cua-driver call bring_to_front '{\"pid\":1}'"], "cua")[0].startswith("foreground"))
        script = "cua-driver call run_script '{\"script\":\"const p={pid:1,delivery_mode:\\\"foreground\\\"};\"}'"
        self.assertTrue(run([script], "cua")[0].startswith("foreground"))

    def test_a_file_is_only_readable_after_the_run_wrote_it(self):
        self.assertEqual(run(["grep x /tmp/s.json", "agent-desktop snapshot > /tmp/s.json", "grep x /tmp/s.json"])[1:],
                         [None, None])
        self.assertIsNotNone(run(["grep x /tmp/s.json"])[0])


class PromptTests(unittest.TestCase):
    def test_v2_prompt_names_only_the_arms_own_foreground_flag(self):
        text = common.native_system_prompt("peekaboo")
        self.assertIn("`--foreground`", text)
        self.assertNotIn("--headed", text)
        self.assertIn("background only", text)
        self.assertNotIn("background only", common.native_system_prompt("ad-14", allow_foreground=True))


if __name__ == "__main__":
    unittest.main()
