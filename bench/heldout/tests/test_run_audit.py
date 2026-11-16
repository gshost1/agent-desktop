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
    def test_reading_the_harness_environment(self):
        for cmd in ["env", "env | grep KEY", "printenv ANTHROPIC_API_KEY", "set", "export -p", "declare -x",
                    "echo $ANTHROPIC_API_KEY", "echo ${CLAUDE_CODE_ENTRYPOINT}", "X=$HO_ARM_HOME; echo $X"]:
            self.assertEqual(run([cmd])[0], "reads the harness environment", cmd)
        for cmd in ["set -e; agent-desktop snapshot --app TextEdit", "export X=1; echo $X",
                    "env LANG=C agent-desktop snapshot --app TextEdit"]:
            self.assertIsNone(run([cmd])[0], cmd)

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


class AuditBypassTests(unittest.TestCase):
    """Astra audit F6: each command here passed the audit before the fix."""

    def test_programs_that_read_a_file_from_inside(self):
        for cmd in ["awk 'BEGIN{while((getline l < \"/etc/hosts\")>0) print l}'",
                    "awk 'BEGIN{system(\"id\")}'", "awk '{print | \"sh\"}' /tmp/x",
                    "jq -n --rawfile t /etc/hosts '$t'", "jq -n --slurpfile t /etc/x.json '$t'",
                    "jq -n 'import \"x\" as $x; $x'", "jq -n '$ENV'", "sed -n '1r /etc/hosts' /dev/null",
                    "awk -f /etc/x.awk"]:
            self.assertIsNotNone(run([cmd])[0], cmd)

    def test_a_leading_or_substituted_input_redirect(self):
        for cmd in ["< /etc/hosts head -3", "X=$(< /etc/hosts)", "echo \"$(</etc/hosts)\""]:
            self.assertIsNotNone(run([cmd])[0], cmd)

    def test_a_guide_path_that_escapes_the_guide(self):
        guide = common.NATIVE_GUIDE_DIR["cua"]
        for cmd in [f"cat {guide}/../../../../etc/hosts", f"head {guide}/../peekaboo/SKILL.md"]:
            self.assertIsNotNone(run([cmd], "cua")[0], cmd)
        self.assertIsNone(run([f"cat {guide}/./SKILL.md"], "cua")[0])

    def test_a_program_hidden_in_a_variable_or_unlisted(self):
        for cmd in ["P=cat; $P /etc/hosts", "$P /etc/hosts", "A=\"agent-desktop press --headed\"; $A cmd+t",
                    "lsappinfo front", "function f { cat /etc/hosts; }; f", "case x in x) cat /etc/hosts;; esac"]:
            self.assertIsNotNone(run([cmd])[0], cmd)

    def test_own_tool_clipboard(self):
        self.assertTrue(run(["peekaboo clipboard get --json"], "peekaboo")[0].startswith("clipboard"))
        self.assertTrue(run(["peekaboo paste \"x\" --app TextEdit"], "peekaboo")[0].startswith("clipboard"))
        self.assertTrue(run(["agent-desktop clipboard-get"])[0].startswith("clipboard"))
        self.assertTrue(run(["cua-driver call clipboard_read '{}'"], "cua")[0].startswith("clipboard"))

    def test_documented_foreground_routes(self):
        steps = "cua-driver call run_steps '{\"steps\":[{\"tool\":\"click\"}],\"foreground_fallback\":true}'"
        self.assertTrue(run([steps], "cua")[0].startswith("foreground"))
        self.assertTrue(run(["peekaboo see --app TextEdit --web-focus --json"], "peekaboo")[0].startswith("foreground"))
        self.assertTrue(run(["peekaboo app switch --to TextEdit"], "peekaboo")[0].startswith("foreground"))
        self.assertTrue(run(["agent-desktop focus-window --app TextEdit"])[0].startswith("foreground"))
        self.assertTrue(run(["agent-desktop launch TextEdit --activate"])[0].startswith("foreground"))


class AuditWriteTests(unittest.TestCase):
    def test_writes_outside_scratch_fail(self):
        for cmd in ["echo x > ~/agent-desktop-bench/seed.game", "agent-desktop snapshot >> /Users/x/notes.txt",
                    "agent-desktop snapshot | tee /etc/x", "echo x > ../../seed.game", "echo x > \"$F\""]:
            self.assertTrue(run([cmd])[0].startswith("writes"), cmd)

    def test_scratch_writes_pass(self):
        for cmd in ["agent-desktop snapshot > /tmp/s.json 2>/dev/null", "agent-desktop snapshot > s.json 2>&1",
                    "agent-desktop snapshot | tee /tmp/a.txt | head", "agent-desktop snapshot > \"$TMPDIR/s.json\""]:
            self.assertIsNone(run([cmd])[0], cmd)


class AuditAllowsOwnWorkTests(unittest.TestCase):
    def test_cat_of_output_the_run_saved(self):
        self.assertEqual(run(["agent-desktop snapshot --app TextEdit --lines > /tmp/own.txt", "cat /tmp/own.txt"]),
                         [None, None])
        self.assertEqual(run(["agent-desktop snapshot > /tmp/own.txt; cat /tmp/own.txt | grep x"]), [None])

    def test_functions_and_literal_variables_the_run_defined(self):
        cmds = ["P() { agent-desktop press \"$@\" --app TextEdit; }; P down", "A=agent-desktop; $A snapshot -i",
                "awk '/\\[2[0-9]\\]|\\[3[0-9]\\]/' /tmp/own.txt", "agent-desktop clipboard-set --help | head"]
        self.assertEqual(run(["agent-desktop snapshot > /tmp/own.txt"] + cmds)[1:], [None] * len(cmds))

    def test_a_regex_alternation_in_awk_is_not_a_pipe(self):
        self.assertIsNone(run(["agent-desktop snapshot --lines | awk -F'\\t' '/a|b/ {print $1 \"|\" $2}'"])[0])


class PromptTests(unittest.TestCase):
    def test_v2_prompt_names_only_the_arms_own_foreground_flag(self):
        text = common.native_system_prompt("peekaboo")
        self.assertIn("`--foreground`", text)
        self.assertNotIn("--headed", text)
        self.assertIn("background only", text)
        self.assertNotIn("background only", common.native_system_prompt("ad-14", allow_foreground=True))


if __name__ == "__main__":
    unittest.main()
