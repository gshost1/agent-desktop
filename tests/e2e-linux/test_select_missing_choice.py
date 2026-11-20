import unittest
from contextlib import contextmanager, nullcontext
from types import SimpleNamespace
from unittest.mock import Mock, patch

from scenarios import select


class MissingChoiceTests(unittest.TestCase):
    @contextmanager
    def fixture(self, mode, *, error=None, mutate=False, combos=None):
        headless = mode == "headless"
        error = error if error is not None else {
            "code": "POLICY_DENIED" if headless else "ELEMENT_NOT_FOUND",
            "details": {"kind": "pointer_required", "physical_delivery_started": False} if headless else
                       {"searched": True, "popup_closed": True},
            "disposition": {"delivery": "not_delivered" if headless else "delivered_unverified",
                            "retry": "safe" if headless else "unsafe"},
        }
        state = ["uint32 300"]
        ctx = Mock(mode=mode, tier=SimpleNamespace(display=None))
        def product(*args, **kwargs):
            if mutate:
                state[0] = "uint32 0"
            return SimpleNamespace(ok=False, error=error, raw="fixture refusal")
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

    def test_both_policies_refuse_the_absent_choice_without_changing_the_setting(self):
        for mode in ("headless", "headed"):
            with self.subTest(mode=mode), self.fixture(mode) as (ctx, state, restore):
                select.absent_choice_refused(ctx)
                self.assertEqual(ctx.product.call_args.args[0], ["select", "@snapshot:e1", "absent-option"])
                self.assertEqual(state[0], "uint32 300")
                restore.assert_not_called()
                ctx.after.assert_called_once_with("gsettings", "uint32 300")
                ctx.restored.assert_called_once_with("gsettings", "uint32 300")

    def test_missing_or_ambiguous_combo_is_not_acted_on(self):
        for combos in ([], [SimpleNamespace(name="Screen Blank"), SimpleNamespace(name="Screen Blank")]):
            with self.subTest(combos=combos), self.fixture("headless", combos=combos) as (ctx, _, _):
                with self.assertRaisesRegex(AssertionError, "missing or ambiguous"):
                    select.absent_choice_refused(ctx)
                ctx.product.assert_not_called()

    def test_unexpected_error_wrong_delivery_and_an_unclosed_popup_still_fail(self):
        errors = [
            {"code": "ACTION_NOT_SUPPORTED"},
            {"code": "ELEMENT_NOT_FOUND", "disposition": {"delivery": "not_delivered", "retry": "safe"}},
            {"code": "ELEMENT_NOT_FOUND", "disposition": {"delivery": "delivered_unverified", "retry": "unsafe"},
             "details": {"searched": True, "popup_closed": False}},
        ]
        for error in errors:
            with self.subTest(error=error), self.fixture("headed", error=error) as (ctx, _, _):
                with self.assertRaises(AssertionError):
                    select.absent_choice_refused(ctx)

    def test_an_unexpected_mutation_fails_and_is_restored(self):
        with self.fixture("headless", mutate=True) as (ctx, state, restore):
            with self.assertRaises(AssertionError):
                select.absent_choice_refused(ctx)
            self.assertEqual(state[0], "uint32 300")
            restore.assert_called_once()
