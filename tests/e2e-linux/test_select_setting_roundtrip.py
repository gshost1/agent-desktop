import unittest
from contextlib import contextmanager, nullcontext
from types import SimpleNamespace
from unittest.mock import Mock, patch

from scenarios import select


class SelectSettingTests(unittest.TestCase):
    @contextmanager
    def fixture(self, mode, *, initial="uint32 300", effect=True, error=None, combos=None):
        state = [initial]
        ctx = Mock(mode=mode, tier=SimpleNamespace(display=None))
        if error is None and mode == "headless":
            error = {"code": "POLICY_DENIED", "details": {"kind": "pointer_required", "physical_delivery_started": False},
                     "disposition": {"delivery": "not_delivered", "retry": "safe"}}
        def product(argv, **kwargs):
            if effect and mode == "headed":
                state[0] = "uint32 0" if argv[-1] == "Never" else "uint32 300"
            return SimpleNamespace(ok=error is None, error=error, raw="fixture response")
        ctx.product.side_effect = product
        def changed(read, expected, **kwargs):
            self.assertEqual(read(), expected)
            return read()
        if combos is None:
            combos = [SimpleNamespace(name="Screen Blank")]
        with patch.object(select, "managed", return_value=nullcontext(object())), \
            patch.object(select, "root", return_value=SimpleNamespace(find=lambda **kw: combos)), \
            patch.object(select, "ref", return_value="@snapshot:e1"), \
            patch.object(select.gsettings, "get", side_effect=lambda *args: state[0]), \
            patch.object(select.gsettings, "set", side_effect=lambda *args: state.__setitem__(0, args[-1])) as restore, \
            patch.object(select, "changed", side_effect=changed):
            yield ctx, state, restore

    def test_headless_refusal_preserves_the_setting_and_headed_success_changes_then_restores_it(self):
        for initial in ("uint32 300", "uint32 0"):
            for mode in ("headless", "headed"):
                with self.subTest(mode=mode, initial=initial), self.fixture(mode, initial=initial) as (ctx, state, restore):
                    select.screen_blank_value(ctx)
                    label, wanted = ("Never", "uint32 0") if initial != "uint32 0" else ("5 minutes", "uint32 300")
                    self.assertEqual(ctx.product.call_args.args[0], ["select", "@snapshot:e1", label])
                    ctx.after.assert_called_once_with("gsettings", initial if mode == "headless" else wanted)
                    ctx.restored.assert_called_once_with("gsettings", initial)
                    self.assertEqual(state[0], initial)
                    self.assertEqual(restore.call_count, int(mode == "headed"))

    def test_missing_or_ambiguous_combo_is_not_acted_on(self):
        for combos in ([], [SimpleNamespace(name="Screen Blank"), SimpleNamespace(name="Screen Blank")]):
            with self.subTest(combos=combos), self.fixture("headless", combos=combos) as (ctx, _, _):
                with self.assertRaisesRegex(AssertionError, "missing or ambiguous"):
                    select.screen_blank_value(ctx)
                ctx.product.assert_not_called()

    def test_success_without_the_setting_change_still_fails(self):
        with self.fixture("headed", effect=False) as (ctx, state, restore):
            with self.assertRaises(AssertionError):
                select.screen_blank_value(ctx)
            self.assertEqual(state[0], "uint32 300")
            restore.assert_not_called()

    def test_a_failure_after_changing_the_setting_is_restored_without_hiding_the_failure(self):
        with self.fixture("headed", error={"code": "ACTION_FAILED"}) as (ctx, state, restore):
            with self.assertRaises(AssertionError):
                select.screen_blank_value(ctx)
            self.assertEqual(state[0], "uint32 300")
            restore.assert_called_once()

    def test_wrong_headless_refusals_are_not_accepted(self):
        for error in ({"code": "ACTION_NOT_SUPPORTED"}, {"code": "POLICY_DENIED"}):
            with self.subTest(error=error), self.fixture("headless", error=error) as (ctx, _, _):
                with self.assertRaises(AssertionError):
                    select.screen_blank_value(ctx)
