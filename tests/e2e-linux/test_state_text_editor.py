import unittest
from contextlib import ExitStack, contextmanager, nullcontext
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import Mock, patch

from scenarios import state


class TextEditorEmptyStartTests(unittest.TestCase):
    @contextmanager
    def fixture(self, mode, startup, failure=None):
        ctx = Mock(mode=mode)
        app = SimpleNamespace(pid=123)
        document = {"text": startup[-1]}
        pending = list(startup)
        commands = []

        def read():
            if pending:
                document["text"] = pending.pop(0)
            return document["text"]

        field = SimpleNamespace(text=Mock(side_effect=read))

        def poll(predicate, *args, **kw):
            for _ in range(4):
                value = predicate()
                if value:
                    return value
            return None

        def find(*args, **kw):
            self.assertEqual(document["text"], "", "wait before allocating the ref")
            return "@s:e1"

        def command(ctx, argv, app):
            commands.append(argv)
            if argv[0] == "type":
                document["text"] = argv[-1]
                if failure == "type":
                    raise RuntimeError("typing failed after writing")
            elif argv[0] == "clear":
                document["text"] = ""
            else:
                raise AssertionError(argv)

        def launch(ctx, name, args):
            self.assertEqual(name, "Text Editor")
            self.assertEqual(args[:2], ("--standalone", "--new-window"))
            self.assertEqual(Path(args[2]).read_bytes(), b"")
            self.assertEqual(Path(args[2]).parent.parent, ctx.tier.runtime_dir)
            return nullcontext(app)

        with ExitStack() as stack:
            ctx.tier.runtime_dir = Path(stack.enter_context(TemporaryDirectory()))
            managed = stack.enter_context(patch.object(state, "managed", side_effect=launch))
            stack.enter_context(patch.object(state, "root", return_value=app))
            stack.enter_context(patch.object(state, "one", return_value=field))
            target = stack.enter_context(patch.object(state, "ref", side_effect=find))
            stack.enter_context(patch.object(state, "command", side_effect=command))
            stack.enter_context(patch("runner.helper.raw.poll", side_effect=poll))
            stale = stack.enter_context(patch.object(
                state, "stale_twin",
                side_effect=RuntimeError("stale check failed") if failure == "stale" else None,
            ))
            yield ctx, app, field, document, commands, managed, target, stale

    def test_private_empty_file_and_readiness_are_required_in_both_modes(self):
        for mode in ("headless", "headed"):
            for startup in (("",), (None, None, ""), ("loading draft", "")):
                with self.subTest(mode=mode, startup=startup), self.fixture(mode, startup) as values:
                    ctx, app, field, document, commands, managed, target, stale = values
                    state.text_editor_semantic_type(ctx)
                    managed.assert_called_once()
                    self.assertFalse(Path(managed.call_args.args[2][2]).exists())
                    target.assert_called_once_with(ctx, app, role="textfield")
                    self.assertEqual(commands, [["type", "@s:e1", "Desktop café"], ["clear", "@s:e1"]])
                    ctx.before.assert_called_once_with("atspi", "")
                    ctx.after.assert_called_once_with("atspi", "Desktop café")
                    ctx.restored.assert_called_once_with("atspi", "")
                    stale.assert_called_once_with(ctx, app, "@s:e1", field.text, ("type", "ignored"))
                    self.assertEqual(document["text"], "")

    def test_nonempty_or_unread_document_fails_before_product_input(self):
        for initial in ("persisted draft", None):
            with self.subTest(initial=initial), self.fixture("headless", (initial,)) as values:
                ctx, _, _, _, commands, managed, target, stale = values
                with self.assertRaisesRegex(AssertionError, "oracle never reached"):
                    state.text_editor_semantic_type(ctx)
                self.assertEqual(commands, [])
                target.assert_not_called()
                stale.assert_not_called()
                ctx.before.assert_not_called()
                self.assertFalse(Path(managed.call_args.args[2][2]).exists())

    def test_typing_and_stale_check_failures_still_clear_the_new_document(self):
        for failure in ("type", "stale"):
            with self.subTest(failure=failure), self.fixture("headed", ("",), failure) as values:
                ctx, _, _, document, commands, managed, _, _ = values
                with self.assertRaises(RuntimeError):
                    state.text_editor_semantic_type(ctx)
                self.assertEqual(commands[-1], ["clear", "@s:e1"])
                self.assertEqual(document["text"], "")
                ctx.restored.assert_called_once_with("atspi", "")
                self.assertFalse(Path(managed.call_args.args[2][2]).exists())
