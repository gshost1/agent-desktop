import unittest
from contextlib import ExitStack, contextmanager
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import Mock, patch

from scenarios import state


class TextEditorEmptyStartTests(unittest.TestCase):
    @contextmanager
    def fixture(self, mode, startup, failure=None, titles=None):
        ctx = Mock(mode=mode)
        app = SimpleNamespace(pid=123)
        document = {"text": startup[-1]}
        pending = list(startup)
        commands = []
        window = SimpleNamespace(role=23, name="")
        app.window = window
        private = {}
        pending_titles = []

        def walk():
            if pending_titles:
                name = pending_titles.pop(0)
                window.name = private["path"].name if name == "expected" else name
            return [window, SimpleNamespace(role=29, name=private["path"].name)]

        tree = SimpleNamespace(walk=Mock(side_effect=walk))

        def read():
            self.assertIn(private["path"].name, window.name, "wait for the file name before reading")
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
            self.assertIn(private["path"].name, window.name)
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
            private["path"] = Path(args[2])
            pending_titles[:] = titles if titles is not None else ("expected",)

            def write_session():
                for folder in ("gnome-text-editor", "org.gnome.TextEditor"):
                    base = Path(ctx.tier.env["XDG_DATA_HOME"]) / folder
                    (base / "drafts").mkdir(parents=True, exist_ok=True)
                    (base / "session.gvariant").write_text(str(private["path"]))
                    (base / "drafts" / "draft").write_text("Desktop café")
                    (base / "recently-used.xbel").write_text("keep")

            @contextmanager
            def running():
                write_session()
                try:
                    yield app
                finally:
                    write_session()

            return running()

        with ExitStack() as stack:
            ctx.tier.runtime_dir = Path(stack.enter_context(TemporaryDirectory()))
            ctx.tier.env = {"XDG_DATA_HOME": str(ctx.tier.runtime_dir / "data")}
            managed = stack.enter_context(patch.object(state, "managed", side_effect=launch))
            stack.enter_context(patch.object(state, "root", return_value=tree))
            stack.enter_context(patch.object(state, "one", return_value=field))
            target = stack.enter_context(patch.object(state, "ref", side_effect=find))
            stack.enter_context(patch.object(state, "command", side_effect=command))
            stack.enter_context(patch("runner.helper.raw.poll", side_effect=poll))
            stale = stack.enter_context(patch.object(
                state, "stale_twin",
                side_effect=RuntimeError("stale check failed") if failure == "stale" else None,
            ))
            yield ctx, app, field, document, commands, managed, target, stale

    def assert_session_removed(self, ctx):
        for folder in ("gnome-text-editor", "org.gnome.TextEditor"):
            base = Path(ctx.tier.env["XDG_DATA_HOME"]) / folder
            self.assertFalse((base / "session.gvariant").exists())
            self.assertFalse((base / "drafts").exists())
            self.assertEqual((base / "recently-used.xbel").read_text(), "keep")

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
                    self.assert_session_removed(ctx)

    def test_empty_document_waits_for_its_window_or_tab_name(self):
        for role in (23, 37, 69):
            with self.subTest(role=role), self.fixture(
                "headless", ("",), titles=("", "another.txt", "expected"),
            ) as values:
                ctx, app, _, _, commands, _, _, _ = values
                app.window.role = role
                state.text_editor_semantic_type(ctx)
                self.assertEqual(commands[0][0], "type")

    def test_an_empty_unnamed_or_wrong_document_never_allocates_a_ref(self):
        for title in ("", "empty.txt", "another.txt"):
            with self.subTest(title=title), self.fixture("headless", ("",), titles=(title,)) as values:
                ctx, _, field, _, commands, _, target, _ = values
                with self.assertRaisesRegex(AssertionError, "oracle never reached"):
                    state.text_editor_semantic_type(ctx)
                target.assert_not_called()
                field.text.assert_not_called()
                self.assertEqual(commands, [])
                self.assert_session_removed(ctx)

    def test_private_file_names_are_unique_between_legs(self):
        with self.fixture("headless", ("",)) as values:
            ctx = values[0]
            names = []
            for _ in range(2):
                with state._empty_editor(ctx) as (_, path):
                    names.append(path.name)
                    self.assertEqual(path.read_bytes(), b"")
                self.assertFalse(path.exists())
            self.assertNotEqual(*names)
            self.assert_session_removed(ctx)

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
                self.assert_session_removed(ctx)

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
                self.assert_session_removed(ctx)

    def test_session_cleanup_errors_fail_a_successful_leg_without_skipping_other_paths(self):
        with self.fixture("headless", ("",)) as values:
            ctx = values[0]
            remove = state.remove

            def denied(path):
                if path.parent.name == "gnome-text-editor" and path.name == "session.gvariant":
                    raise PermissionError("session denied")
                remove(path)

            with patch.object(state, "remove", side_effect=denied) as cleanup:
                with self.assertRaisesRegex(AssertionError, "Text Editor session cleanup: .*session denied"):
                    state.text_editor_semantic_type(ctx)
            self.assertEqual(cleanup.call_count, 4)
            base = Path(ctx.tier.env["XDG_DATA_HOME"])
            self.assertFalse((base / "gnome-text-editor" / "drafts").exists())
            self.assertFalse((base / "org.gnome.TextEditor" / "session.gvariant").exists())
            self.assertFalse((base / "org.gnome.TextEditor" / "drafts").exists())

    def test_session_cleanup_errors_are_recorded_without_masking_the_leg_failure(self):
        with self.fixture("headed", ("",), "type") as values:
            ctx = values[0]
            with patch.object(state, "remove", side_effect=PermissionError("session denied")) as cleanup:
                with self.assertRaisesRegex(RuntimeError, "typing failed after writing"):
                    state.text_editor_semantic_type(ctx)
            self.assertEqual(cleanup.call_count, 4)
            ctx.finding.assert_called_once()
            self.assertIn("Text Editor session cleanup:", ctx.finding.call_args.args[0])
            self.assertIn("session denied", ctx.finding.call_args.args[0])
