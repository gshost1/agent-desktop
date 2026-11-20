import unittest
from contextlib import nullcontext
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from runner import helper
from scenarios import toolkit_qt, values


class ListedAppNameTests(unittest.TestCase):
    def test_lookup_uses_the_owned_pid_not_the_accessibility_name(self):
        app = SimpleNamespace(pid=73, name="qt_window.py")
        inventory = {"apps": [{"pid": 74, "name": "other"}, {"pid": 73, "name": "python3.12"}]}
        with patch.object(helper, "command", return_value=SimpleNamespace(data=inventory)) as command:
            self.assertEqual(helper.listed_app_name(object(), app), "python3.12")
        self.assertEqual(command.call_args.args[1], ["list-apps"])

    def test_missing_duplicated_or_unnamed_owned_process_is_refused(self):
        app = SimpleNamespace(pid=73)
        for rows in ([], [{"pid": 74, "name": "other"}],
                     [{"pid": 73, "name": "one"}, {"pid": 73, "name": "two"}],
                     [{"pid": 73}], [{"pid": 73, "name": "  "}]):
            with self.subTest(rows=rows), \
                patch.object(helper, "command", return_value=SimpleNamespace(data={"apps": rows})):
                with self.assertRaises(AssertionError):
                    helper.listed_app_name(object(), app)

    def test_inventory_errors_are_not_replaced_with_an_accessibility_name(self):
        with patch.object(helper, "command", side_effect=AssertionError("inventory failed")):
            with self.assertRaisesRegex(AssertionError, "inventory failed"):
                helper.listed_app_name(object(), SimpleNamespace(pid=73, name="qt_window.py"))

    def test_qt_wait_uses_the_listed_name_for_both_predicates(self):
        ctx, app = object(), SimpleNamespace(name="qt_window.py")
        for opened in (False, True):
            with self.subTest(opened=opened), \
                patch.object(toolkit_qt, "listed_app_name", return_value="python3.12"), \
                patch.object(toolkit_qt, "command") as command:
                toolkit_qt.wait_menu(ctx, app, opened, timeout=123)
                self.assertEqual(command.call_args.args[1], [
                    "wait", "--menu" if opened else "--menu-closed", "--app", "python3.12", "--timeout", "123"])

    def test_terminal_waits_use_the_listed_name_including_restoration(self):
        app, ctx = object(), Mock()
        states = set()
        waits = []
        def command(ctx, argv, *args):
            if argv[0] == "wait":
                waits.append(argv[argv.index("--app") + 1])
            elif argv == ["click", "Read-Only"]:
                states.symmetric_difference_update({"CHECKED"})
        with patch.object(values, "managed", return_value=nullcontext(app)), \
            patch.object(values, "listed_app_name", return_value="gnome-terminal-server"), \
            patch.object(values, "root"), \
            patch.object(values, "one", return_value=SimpleNamespace(states=states)), \
            patch.object(values, "ref", side_effect=lambda *a, **kw: kw["name"]), \
            patch.object(values, "command", side_effect=command), \
            patch.object(values, "changed", side_effect=lambda read, expected: self.assertEqual(read(), expected)), \
            patch.object(values, "stale_twin"):
            values.terminal_read_only_flip(ctx)
        self.assertEqual(waits, ["gnome-terminal-server", "gnome-terminal-server"])
        self.assertEqual(states, set())

    def test_qt_cleanup_waits_for_exact_membership_and_still_refuses_a_persistent_leak(self):
        baseline = {"members": [(1, 2, "service")], "log": None}
        pending = {"members": [(1, 2, "service"), (3, 4, "holder")], "log": None}
        for settles in (True, False):
            with self.subTest(settles=settles):
                ctx = Mock(tier=SimpleNamespace(runtime_dir=Path("/private/test")))
                reads = [baseline, pending, baseline if settles else pending]
                def read(_):
                    return reads.pop(0) if len(reads) > 1 else reads[0]
                def poll(predicate, *args, **kw):
                    return any(predicate() for _ in range(2))
                with patch.object(toolkit_qt, "log_state", side_effect=read), \
                    patch.object(toolkit_qt, "remove"), \
                    patch.object(toolkit_qt, "managed", return_value=nullcontext(object())), \
                    patch.object(helper.raw, "poll", side_effect=poll):
                    if settles:
                        with toolkit_qt.qt_app(ctx):
                            pass
                        ctx.restored.assert_called_once_with("applog", baseline)
                    else:
                        with self.assertRaises(AssertionError):
                            with toolkit_qt.qt_app(ctx):
                                pass
                        ctx.restored.assert_not_called()
