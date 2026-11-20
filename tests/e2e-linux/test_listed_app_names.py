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
        ctx = object()
        for opened in (False, True):
            with self.subTest(opened=opened), \
                patch.object(toolkit_qt, "listed_app_name", return_value="python3.12"), \
                patch.object(toolkit_qt, "command") as command:
                toolkit_qt.wait_menu(ctx, "python3.12", opened, timeout=123)
                toolkit_qt.listed_app_name.assert_not_called()
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

    def test_qt_transition_resolves_once_before_any_wait_timer_or_trigger(self):
        for opening, already in ((True, True), (True, False), (False, False)):
            with self.subTest(opening=opening, already=already):
                ctx = Mock(tier=SimpleNamespace(runtime_dir=Path("/private/test")))
                order = []
                def lookup(*args):
                    order.append("lookup")
                    return "python3.12"
                def wait(ctx, name, opened, **kwargs):
                    self.assertEqual(name, "python3.12")
                    order.append("wait")
                def clock():
                    order.append("clock")
                    return 0
                def delayed(*args):
                    order.append("trigger")
                    return Mock()
                after = ["menu-open"] if opening else ["menu-open", "menu-closed"]
                with patch.object(toolkit_qt, "qt_app", return_value=nullcontext(object())), \
                    patch.object(toolkit_qt, "listed_app_name", side_effect=lookup) as lookup_mock, \
                    patch.object(toolkit_qt, "root"), \
                    patch.object(toolkit_qt, "menu_open", return_value=False), \
                    patch.object(toolkit_qt, "events", side_effect=[[], [], after]), \
                    patch.object(toolkit_qt, "poll", return_value=True), \
                    patch.object(toolkit_qt, "open_raw"), \
                    patch.object(toolkit_qt, "log_state"), \
                    patch.object(toolkit_qt, "wait_menu", side_effect=wait), \
                    patch.object(toolkit_qt.time, "monotonic", side_effect=clock), \
                    patch.object(toolkit_qt, "delayed", side_effect=delayed):
                    toolkit_qt.menu_transition(ctx, opening, already=already)
                lookup_mock.assert_called_once()
                self.assertEqual(order[0], "lookup")
                self.assertIn("clock" if already else "trigger", order)

    def test_qt_cleanup_requires_exact_membership_immediately(self):
        baseline = {"members": [(1, 2, "service")], "log": None}
        pending = {"members": [(1, 2, "service"), (3, 4, "holder")], "log": None}
        for settles in (True, False):
            with self.subTest(settles=settles):
                ctx = Mock(tier=SimpleNamespace(runtime_dir=Path("/private/test")))
                with patch.object(toolkit_qt, "log_state", side_effect=[baseline, baseline if settles else pending]), \
                    patch.object(toolkit_qt, "remove"), \
                    patch.object(toolkit_qt, "managed", return_value=nullcontext(object())), \
                    patch.object(helper.raw, "poll", side_effect=AssertionError("cleanup must not poll")):
                    if settles:
                        with toolkit_qt.qt_app(ctx):
                            pass
                        ctx.restored.assert_called_once_with("applog", baseline)
                    else:
                        with self.assertRaises(AssertionError):
                            with toolkit_qt.qt_app(ctx):
                                pass
                        ctx.restored.assert_not_called()
