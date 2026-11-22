from contextlib import contextmanager
from pathlib import Path
from tempfile import TemporaryDirectory

from runner.helper import (changed, command, listed_app_name, managed, one, ref, remove,
                           require, root, stale_twin)
from runner.legs import implementation
from runner.verdict import LegContext


@implementation("state.terminal_copy_follows_selection")
def terminal_copy_follows_selection(ctx: LegContext) -> None:
    with managed(ctx, "Terminal", ("--show-menubar",)) as app:
        name = listed_app_name(ctx, app)
        tree = root(ctx, app)
        terminal = one(tree, role=60)
        copy = one(tree, name="Copy", role=35)
        menu = ref(ctx, app, name="Edit", role="menu")
        interface = "org.a11y.atspi.Text"
        length = terminal.get_property(interface, "CharacterCount")
        require(length > 0 and "SENSITIVE" not in copy.states, "Copy not initially disabled")
        ctx.before("atspi", "SENSITIVE" in copy.states)
        try:
            command(ctx, ["click", menu], app)
            command(ctx, ["wait", "--menu", "--app", name, "--timeout", "5000"])
            target = ref(ctx, app, name="Copy", role="menuitem")
            require(command(ctx, ["is", target, "--property", "enabled"], app).data["result"] is False)
            require(terminal.invoke(interface, "AddSelection", "(ii)", (0, length))[0])
            changed(lambda: "SENSITIVE" in copy.states, True)
            require(command(ctx, ["is", target, "--property", "enabled"], app).data["result"] is True)
            ctx.after("atspi", "SENSITIVE" in copy.states)
        finally:
            try:
                count = terminal.invoke(interface, "GetNSelections")[0]
                for index in reversed(range(count)):
                    require(terminal.invoke(interface, "RemoveSelection", "(i)", (index,))[0])
                changed(lambda: "SENSITIVE" in copy.states, False)
                ctx.restored("atspi", "SENSITIVE" in copy.states)
            finally:
                if "SHOWING" in copy.states:
                    command(ctx, ["press", "escape"], app, headed=True)
                changed(lambda: "SHOWING" in copy.states, False)


@contextmanager
def _empty_editor(ctx):
    data_home = Path(ctx.tier.env["XDG_DATA_HOME"])
    succeeded = False
    try:
        with TemporaryDirectory(prefix="semantic-type-", dir=ctx.tier.runtime_dir) as directory:
            directory = Path(directory)
            document = directory / f"{directory.name}.txt"
            document.touch()
            with managed(ctx, "Text Editor", ("--standalone", "--new-window", str(document))) as app:
                yield app, document
        succeeded = True
    finally:
        problems = []
        for folder in ("gnome-text-editor", "org.gnome.TextEditor"):
            for name in ("session.gvariant", "drafts"):
                path = data_home / folder / name
                try:
                    remove(path)
                except Exception as error:
                    problems.append(f"{path}: {error}")
        if problems:
            message = "Text Editor session cleanup: " + "; ".join(problems)
            if succeeded:
                raise AssertionError(message)
            ctx.finding(message)


@implementation("state.text_editor_semantic_type")
def text_editor_semantic_type(ctx: LegContext) -> None:
    with _empty_editor(ctx) as (app, document):
        changed(lambda: any(document.name in node.name
                            for node in root(ctx, app).walk()
                            if node.role in (23, 37, 69)), True)
        tree = root(ctx, app)
        field = one(tree, role=61)
        initial = changed(field.text, "")
        require(initial == "", "document must start empty")
        target = ref(ctx, app, role="textfield")
        ctx.before("atspi", initial)
        try:
            command(ctx, ["type", target, "Desktop café"], app)
            changed(field.text, "Desktop café")
            ctx.after("atspi", field.text())
            stale_twin(ctx, app, target, field.text, ("type", "ignored"))
        finally:
            command(ctx, ["clear", target], app)
            changed(field.text, initial)
            ctx.restored("atspi", field.text())
