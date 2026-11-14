"""Unit tests of the harness: the forbidden-command audit, the report's statistics and ranking, the native
bench-app hygiene and the re-audit. No GUI, no network.

Run: python3.11 -I -m unittest discover -s heldout/tests -p 'test_*.py'
"""
import contextlib
import io
import json
import signal
import sys
import tempfile
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(HELDOUT))
sys.path.insert(0, str(HELDOUT / "arms"))
import common  # noqa: E402
import desktop  # noqa: E402
import reaudit  # noqa: E402
import report  # noqa: E402
import shellwords  # noqa: E402
import tasks_heldout as th  # noqa: E402


def audit(cmd, arm="ad-14", skills=None):
    return common.audit_native(cmd, arm, th.FORBIDDEN, skills)


class ShellWordsTests(unittest.TestCase):
    def progs(self, cmd):
        return [w[0] for w in shellwords.commands(cmd)]

    def test_pipes_inside_quoted_patterns_are_not_separators(self):
        self.assertEqual(self.progs("agent-desktop snapshot | grep -E 'strike| find |cat'"), ["agent-desktop", "grep"])
        self.assertEqual(self.progs('agent-desktop find --name "a; ls -la" | head'), ["agent-desktop", "head"])
        self.assertEqual(self.progs("grep -E \"Format|ls\" x || true"), ["grep", "true"])

    def test_substitution_is_read_inside_double_quotes(self):
        self.assertEqual(sorted(self.progs('echo "id: $(cat /tmp/s)"')), ["cat", "echo"])
        self.assertEqual(sorted(self.progs("echo `ls /`")), ["echo", "ls"])
        self.assertEqual(self.progs("echo '$(cat /tmp/s)'"), ["echo"])

    def test_heredoc_body_is_not_commands(self):
        cmd = "cat > /tmp/s.sh <<'EOF'\nls -la\nopen x\nEOF\nagent-desktop snapshot"
        self.assertEqual(self.progs(cmd), ["cat", "agent-desktop"])

    def test_prefixes_keywords_and_redirects_are_skipped(self):
        self.assertEqual(self.progs("FOO=1 env -i BAR=2 python3 -c 1"), ["python3"])
        self.assertEqual(self.progs("for i in 1 2; do agent-desktop press left; done"), ["agent-desktop"])
        self.assertEqual(self.progs("ls | xargs -n1 cat"), ["ls", "cat"])
        self.assertEqual(self.progs("2>/dev/null agent-desktop snapshot >out 2>&1 &"), ["agent-desktop"])
        self.assertEqual(self.progs("mk(){ python3 -c 1; }; mk"), ["mk", "python3", "mk"])

    def test_and_or_and_background(self):
        self.assertEqual(self.progs("a && b || c; d & e"), ["a", "b", "c", "d", "e"])


class AuditTests(unittest.TestCase):
    def test_quoted_pipe_no_longer_false_fails(self):
        self.assertIsNone(audit("agent-desktop snapshot --app TextEdit | grep -i -E 'strike| find |format'"))
        self.assertIsNone(audit("agent-desktop find --app Preview --name 'Adjust; open' | jq ."))
        self.assertIsNone(audit('agent-desktop snapshot | grep -E "Format| cat |ls"'))

    def test_real_uses_still_fail(self):
        self.assertEqual(audit("agent-desktop click $(cat /tmp/ref)"), "cat")
        self.assertEqual(audit("cat > /tmp/try.sh <<'EOF'\nagent-desktop press left\nEOF\nbash /tmp/try.sh"), "cat")
        self.assertEqual(audit("agent-desktop snapshot | xargs open"), "open")
        self.assertEqual(audit('echo "$(osascript -e 1)"'), "osascript")
        self.assertEqual(audit("cd /tmp && /bin/ls"), "ls")

    def test_cat_of_the_guide_with_a_redirect(self):
        guide = common.NATIVE_GUIDE_DIR["peekaboo"] / "SKILL.md"
        self.assertIsNone(audit(f"cat {guide} 2>/dev/null | head -50", "peekaboo"))
        self.assertIsNone(audit(f"cat '{guide}'", "peekaboo"))
        self.assertEqual(audit(f"cat {guide.parent}-evil/SKILL.md", "peekaboo"), "cat")

    def test_skills_root_of_another_host(self):
        root = Path("/Users/someone/bench-tools/home/.agent-desktop-bench-target/heldout-tools/skills")
        self.assertIsNone(audit(f"cat {root}/peekaboo/SKILL.md", "peekaboo", root))
        self.assertEqual(audit(f"cat {root}/peekaboo/SKILL.md", "peekaboo"), "cat")


def row(arm, task, ok, cli, wall, tok=1000, turns=3, suite="native"):
    return report.normalize({"suite": suite, "arm": arm, "task": task, "model": "m", "success": ok, "cli_s": cli,
                             "wall_s": wall, "input_total": tok, "output_tokens": 0, "num_turns": turns, "verify": "x"})


class ReportTests(unittest.TestCase):
    def hn6_like(self):
        """ad-up fails fast (1/5 at ~24 s), ad-14 passes 4/5 at ~50 s, as on hn6."""
        fast = [row("ad-up", "hn6", i == 0, 4.5, 24) for i in range(5)]
        slow = [row("ad-14", "hn6", i < 4, 6.2, 50) for i in range(5)]
        return fast + slow

    def test_failing_fast_does_not_rank_first(self):
        ranked, _, _ = report.ranking(self.hn6_like(), "native")
        self.assertEqual([x["arm"] for x in ranked], ["ad-14", "ad-up"])
        per = {x["arm"]: x["per"]["wall"] for x in ranked}
        self.assertEqual(per["ad-up"], 120.0)
        self.assertEqual(per["ad-14"], 62.5)

    def test_one_more_failure_never_improves_per_success(self):
        base = [row("a", "t", True, 1, 10) for _ in range(4)]
        worse = base + [row("a", "t", False, 0.1, 1)]
        self.assertLess(report.cell_stats(base, "wall")["per_success"], report.cell_stats(worse, "wall")["per_success"])

    def test_statistics_are_separate(self):
        rs = [row("a", "t", ok, 1, w) for ok, w in [(True, 10), (True, 20), (False, 1), (False, 2), (True, 30)]]
        s = report.cell_stats(rs, "wall")
        self.assertEqual(s["pass_med"], 20)
        self.assertEqual(s["all_med"], 10)
        self.assertEqual(s["max"], 30)
        self.assertAlmostEqual(s["p90"], 26.0)
        self.assertAlmostEqual(s["per_success"], 21.0)

    def test_no_pass_has_no_pass_median(self):
        s = report.cell_stats([row("a", "t", False, 1, 5)], "wall")
        self.assertIsNone(s["pass_med"])
        self.assertIsNone(s["per_success"])

    def test_losses_skip_faster_but_less_reliable_arms(self):
        lost = report.loss_rows(self.hn6_like(), "ad-14")
        self.assertEqual(lost, [])
        lost = report.loss_rows(self.hn6_like(), "ad-up")
        self.assertIn(("native", "hn6", "pass", "1/5", "ad-14", "4/5"), lost)

    def test_losses_list_pass_and_metric(self):
        rows = [row("ad-14", "hw1", True, 1, 10, suite="web"), row("ad-14", "hw1", False, 3, 30, suite="web"),
                row("bh", "hw1", True, 0.5, 12, suite="web"), row("bh", "hw1", True, 0.7, 14, suite="web")]
        text = "\n".join(report.losses(rows))
        self.assertIn("| web | hw1 | pass | 1/2 | bh | 2/2 |", text)
        self.assertIn("| web | hw1 | CLI s | 1.0 | bh |", text)
        self.assertNotIn("wall s", text)

    def test_render_separates_and_labels(self):
        text = report.render(self.hn6_like(), 0)
        self.assertIn("ranking (cannot reward failing)", text)
        self.assertIn("pass median / all median / p90 / max", text)
        self.assertIn("rewards fast failures", text)
        header = next(line for line in text.splitlines() if line.startswith("| rank |"))
        self.assertLess(header.index("CLI"), header.index("wall"))


class FakeDesktop(desktop.Desktop):
    """Scripted app: window titles per query, whether quit works, and whether a menu blocks quit until Escape."""

    def __init__(self, titles, quits=True, menu_open=False, killable=True):
        super().__init__(self.fake_macctl, sleep=lambda s: None)
        self.titles = list(titles)
        self.running = True
        self.quits = quits
        self.menu_open = menu_open
        self.killable = killable
        self.calls = []
        self.signals = []

    def fake_macctl(self, *args, timeout=20):
        self.calls.append(args)
        if args[0] == "windows":
            t = self.titles.pop(0) if len(self.titles) > 1 else self.titles[0]
            return "" if t is None else json.dumps(t)
        if args[0] == "keypid":
            self.menu_open = False
        if args[0] == "quit" and self.quits and not self.menu_open:
            self.running = False
        return ""

    def pids(self, app):
        return [4242] if self.running else []

    def kill(self, pid, sig):
        self.signals.append(sig)
        if self.killable:
            self.running = False


class CloseBenchAppTests(unittest.TestCase):
    def test_leftover_panels_are_closed_not_refused(self):
        d = FakeDesktop([["seed-ho-strike-1.rtf", "Fonts", "Colors"]])
        desktop.close_app(d, "TextEdit")
        self.assertFalse(d.running)
        self.assertIn(("close-discard", "TextEdit", "Colors", "Fonts"), d.calls)
        self.assertEqual(d.signals, [])

    def test_preview_adjust_color_panel(self):
        d = FakeDesktop([["seed-ho-flip-2.png", "Adjust Color"]])
        desktop.close_app(d, "Preview")
        self.assertFalse(d.running)

    def test_foreign_window_is_refused_and_untouched(self):
        d = FakeDesktop([["seed-ho-strike-1.rtf", "Shopping list.rtf"]])
        with self.assertRaisesRegex(RuntimeError, "not a bench window"):
            desktop.close_app(d, "TextEdit")
        self.assertTrue(d.running)
        self.assertEqual([c[0] for c in d.calls], ["windows"])

    def test_stuck_menu_is_escaped_before_quit(self):
        d = FakeDesktop([["seed-ho-strike-1.rtf"]], menu_open=True)
        desktop.close_app(d, "TextEdit")
        self.assertFalse(d.running)
        names = [c[0] for c in d.calls]
        self.assertLess(names.index("keypid"), names.index("quit"))
        self.assertEqual(d.signals, [])

    def test_quit_that_does_not_take_is_forced_after_a_recheck(self):
        d = FakeDesktop([["seed-ho-strike-1.rtf"]], quits=False)
        desktop.close_app(d, "TextEdit")
        self.assertFalse(d.running)
        self.assertEqual(d.signals, [signal.SIGTERM])

    def test_no_force_when_a_foreign_window_appears_before_the_force(self):
        d = FakeDesktop([["seed-ho-a.rtf"], ["seed-ho-a.rtf"], ["Untitled"]], quits=False)
        with self.assertRaisesRegex(RuntimeError, "not a bench window"):
            desktop.close_app(d, "TextEdit")
        self.assertEqual(d.signals, [])

    def test_busy_app_is_waited_for_then_refused(self):
        d = FakeDesktop([None, None, ["seed-ho-a.rtf"]])
        desktop.close_app(d, "TextEdit")
        self.assertFalse(d.running)
        d = FakeDesktop([None])
        with self.assertRaisesRegex(RuntimeError, "does not answer"):
            desktop.close_app(d, "TextEdit", wait_s=6)
        self.assertTrue(d.running)

    def test_not_running_is_a_no_op(self):
        d = FakeDesktop([[]])
        d.running = False
        desktop.close_app(d, "TextEdit")
        self.assertEqual(d.calls, [])

    def test_install_replaces_the_frozen_function(self):
        original = th.close_bench_app
        try:
            desktop.install(th)
            self.assertIsNot(th.close_bench_app, original)
        finally:
            th.close_bench_app = original


class ReauditTests(unittest.TestCase):
    def rows(self):
        def r(arm, task, ok, verify, cmds):
            return {"suite": "native", "arm": arm, "task": task, "success": ok, "verify": verify, "round": 0,
                    "run_id": f"{arm}-{task}", "steps": [{"tools": [{"cmd": c} for c in cmds]}]}
        return [r("ad-14", "hn2", True, "every paragraph struck", ["agent-desktop snapshot"]),
                r("ad-14", "hn2", False, "forbidden command ls; every paragraph struck",
                  ["agent-desktop snapshot | grep -E 'strike| ls '"]),
                r("ad-14", "hn2", False, "forbidden command cat; the document was not saved",
                  ["agent-desktop click $(cat /tmp/r)"]),
                r("ad-up", "hn1", False, "wrong subscript", ["mk(){ python3 -c 1; }; mk"])]

    def test_changes_are_listed_and_rejudged(self):
        changes = reaudit.reaudit(self.rows(), None)
        got = {(c[0]["arm"], c[0]["task"], c[1], c[2], c[4]) for c in changes}
        self.assertEqual(got, {("ad-14", "hn2", "ls", None, True), ("ad-up", "hn1", None, "python3", False)})

    def test_write_keeps_every_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp) / "in.jsonl", Path(tmp) / "out.jsonl"
            src.write_text("".join(json.dumps(r) + "\n" for r in self.rows()))
            sys.argv = ["reaudit.py", str(src), "--write", str(dst)]
            with contextlib.redirect_stdout(io.StringIO()):
                reaudit.main()
            out = [json.loads(line) for line in dst.read_text().splitlines()]
        self.assertEqual(len(out), 4)
        self.assertTrue(out[1]["success"])
        self.assertEqual(out[1]["verify"], "every paragraph struck")


class CandidateArmTests(unittest.TestCase):
    def test_candidate_registers_like_ad14_and_gets_a_logging_wrapper(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "ad-x").mkdir()
            (root / "ad-x" / "agent-desktop").write_text("")
            (root / "ad-x" / "VERSION").write_text("agent-desktop 1.4.0 candidate ad-x sha256 abc\n")
            (root / "ad-14").mkdir()
            (root / "ad-14" / "agent-desktop").write_text("")
            (root / "empty").mkdir()
            try:
                self.assertEqual(common.register_candidates(root), ["ad-x"])
                self.assertEqual(common.NATIVE_TOOL["ad-x"], "agent-desktop")
                self.assertEqual(common.WEB_ARMS["ad-x"], "ad-web")
                self.assertEqual(common.native_system_prompt("ad-x"), common.native_system_prompt("ad-14"))
                self.assertIn("sha256 abc", common.ARM_VERSIONS["ad-x"])
                self.assertNotIn("ad-14", common.CANDIDATE_ARMS)
                bin_dir = common.arm_bin_dir("ad-x", root)
                wrapper = (bin_dir / "agent-desktop").read_text()
                self.assertIn(str(common.HERE / "logwrap.zsh"), wrapper)
                self.assertEqual(common.arm_bin_dir("ad-14", root), common.HERE / "bin" / "ad-14")
            finally:
                common.CANDIDATE_ARMS.discard("ad-x")
                for table in (common.NATIVE_TOOL_TEXT, common.NATIVE_TOOL, common.WEB_ARMS, common.WEB_TOOL,
                              common.ARM_VERSIONS):
                    table.pop("ad-x", None)


if __name__ == "__main__":
    unittest.main()
