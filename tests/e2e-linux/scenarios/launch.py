from __future__ import annotations

from pathlib import Path
from shutil import which

from scenarios.headed_actions import count_name, find_ref as qualified_ref

from oracle import atspi_raw as raw
from runner import apps
from runner.apps import launch, stop, window_id
from runner.helper import changed, require
from runner.legs import implementation
from runner.verdict import LegContext
from oracle.atspi_raw import app_root, registry
from oracle.proc import facts, members

CALCULATOR = "Calculator"
TEXT_EDITOR = "Text Editor"
TERMINAL = "Terminal"


def error_code(envelope) -> str:
    return str((envelope.error or {}).get("code", ""))


@implementation("launch.launch_calculator_attributed")
def launch_calculator_attributed(ctx: LegContext) -> None:
    before = members(ctx.tier.runtime_dir)
    ctx.before("proc.members", before)
    envelope = ctx.product(["launch", CALCULATOR])
    assert envelope.ok, envelope.raw
    launched = (envelope.data or {}).get("pid")
    assert isinstance(launched, int), envelope.raw
    after = members(ctx.tier.runtime_dir)
    ctx.after("proc.members", after)
    assert any(member[0] == launched for member in after), "the launched pid is not a member"
    window = ctx.product(["list-windows", "--app", CALCULATOR])
    assert window.ok and window.data, window.raw
    assert app_root(ctx.tier, "gnome-calculator")
    closed = ctx.product(["close-app", CALCULATOR])
    assert closed.ok, closed.raw
    ctx.restored("proc.members", changed(lambda: members(ctx.tier.runtime_dir), before, seconds=6))


def _window_ids(tier):
    return sorted(window_id(node) for node in app_root(tier, "gnome-text-editor").children()
                  if node.role in (16, 23, 69))


def _close_added_windows(tier, existing):
    for frame in app_root(tier, "gnome-text-editor").children():
        if frame.role in (16, 23, 69) and window_id(frame) not in existing:
            close = frame.find(role=43, name="Close")
            assert len(close) == 1, "new editor frame must have one Close action"
            assert close[0].do_action("click"), "new editor frame did not close"
    return changed(lambda: _window_ids(tier), existing)


@implementation("launch.launch_text_editor_new_window")
def launch_text_editor_new_window(ctx: LegContext) -> None:
    first = launch(ctx.tier, TEXT_EDITOR)
    try:
        existing = _window_ids(ctx.tier)
        ctx.before("atspi.window_ids", existing)
        envelope = ctx.product(["launch", TEXT_EDITOR, "--arg=--new-window"])
        assert envelope.ok, envelope.raw
        after = _window_ids(ctx.tier)
        ctx.after("atspi.window_ids", after)
        assert set(after) - set(existing), "a second launch added no window"
        ctx.restored("atspi.window_ids", _close_added_windows(ctx.tier, existing))
    finally:
        stop(first)


def unresolved_handoff_pid(envelope):
    error = envelope.error or {}
    details = error.get("details", {})
    require(not envelope.ok and envelope.exit_code == 1 and error.get("code") == "ACTION_FAILED",
            "unresolved handoff must report ACTION_FAILED, not a successful launch")
    require(details.get("kind") == "exited" and details.get("status") == 0,
            "the forwarding launcher must have exited successfully")
    require(error.get("disposition") == {"delivery": "delivered_unverified", "retry": "unsafe"},
            "a launch with an unproven service owner must not be retried safely")
    require("recovery" not in error, "an unresolved launch must not promise automatic recovery")
    pid = details.get("spawned_pid")
    require(type(pid) is int and pid > 0, "unresolved handoff must report the spawned pid")
    return pid


@implementation("launch.unresolved_terminal_handoff")
def unresolved_terminal_handoff(ctx: LegContext) -> None:
    read = lambda: [member[2] for member in members(ctx.tier.runtime_dir) if "gnome-terminal" in member[2]]
    before = read()
    owners = lambda: apps.owners_named(ctx.tier, TERMINAL)
    require(not owners(), "the handoff fixture must start without a resident Terminal")
    native = lambda: tuple((app.pid, app.start_time, tuple(app.windows)) for app in owners())
    ctx.before("proc.terminal_owner", before)
    ctx.before("atspi.terminal_owner", ())
    succeeded = False
    try:
        spawned = unresolved_handoff_pid(ctx.product(["launch", TERMINAL, "--timeout", "5000"]))
        found = raw.poll(lambda: [app for app in owners() if app.windows], 5)
        require(found and len(found) == 1, "launch must create one independently observed service window")
        owner = found[0]
        launcher = which(apps.COMMANDS[TERMINAL], path=ctx.tier.env["PATH"])
        require(launcher, "Terminal launcher is missing")
        require(owner.pid != spawned and Path(f"/proc/{owner.pid}/exe").resolve() != Path(launcher).resolve(),
                "the service must have a different executable identity from its launcher")
        require(facts(owner.pid)[3].get("AGENT_DESKTOP_LAUNCHED") != "1",
                "a marked descendant is not the unproven D-Bus handoff fixture")
        inventory = ctx.product(["list-apps"])
        require(inventory.ok and any(app.get("pid") == owner.pid
                                    for app in (inventory.data or {}).get("apps", ())),
                "the healthy native service must still be listed independently of launch attribution")
        ctx.after("proc.terminal_owner", read())
        ctx.after("atspi.terminal_owner", native())
        succeeded = True
    finally:
        problems = _cleanup_handoff(ctx, owners, (
            ("atspi.terminal_owner", native, ()),
            ("proc.terminal_owner", read, before),
        ))
        if problems:
            message = "Terminal cleanup: " + "; ".join(problems)
            if succeeded:
                raise AssertionError(message)
            ctx.finding(message)


def _cleanup_handoff(ctx, owners, observations):
    problems = []
    try:
        remaining = owners()
    except Exception as error:
        problems.append(str(error))
        remaining = []
    for owner in remaining:
        try:
            inventory = ctx.product(["list-apps"])
            require(inventory.ok, inventory.raw)
            matches = [app["name"] for app in (inventory.data or {}).get("apps", ())
                       if app.get("pid") == owner.pid]
            require(len(matches) == 1, "cannot identify the private Terminal service for cleanup")
            closed = ctx.product(["close-app", matches[0]])
            require(closed.ok, closed.raw)
        except Exception as error:
            problems.append(str(error))
    for channel, read, expected in observations:
        try:
            ctx.restored(channel, changed(read, expected, seconds=6))
        except Exception as error:
            problems.append(str(error))
    return problems


@implementation("launch.list_apps_alias")
def list_apps_alias(ctx: LegContext) -> None:
    app = launch(ctx.tier, CALCULATOR)
    read = lambda: sorted(node.name for node in registry(ctx.tier))
    try:
        before = read()
        ctx.before("atspi.apps", before)
        expected = [(app_root(ctx.tier, "gnome-calculator").name, app.pid)]
        for alias in (CALCULATOR, "calculator"):
            envelope = ctx.product(["list-apps", "--app", alias])
            assert envelope.ok, envelope.raw
            data = envelope.data or {}
            assert [(row.get("name"), row.get("pid")) for row in data.get("apps", [])] == expected, envelope.raw
            assert data.get("complete") is not False, envelope.raw
        ctx.after("atspi.apps", read())
        assert read() == before, "query changed registered applications"
        ctx.restored("atspi.apps", read())
    finally:
        stop(app)


@implementation("launch.launch_suggestion")
def launch_suggestion(ctx: LegContext) -> None:
    read = lambda: members(ctx.tier.runtime_dir)
    ctx.before("proc.apps", read())
    envelope = ctx.product(["launch", "No Such Application For Harness"])
    ctx.after("proc.apps", read())
    assert not envelope.ok, envelope.raw
    assert error_code(envelope) == "APP_NOT_FOUND", envelope.raw
    suggestion = (envelope.error or {}).get("suggestion")
    assert suggestion, "the APP_NOT_FOUND envelope carries no suggestion"
    ctx.restored("proc.apps", read())


@implementation("launch.recovery_strategy_on_error")
def recovery_strategy_on_error(ctx: LegContext) -> None:
    app = launch(ctx.tier, CALCULATOR)
    snapshot = ctx.product(["snapshot", "--app", CALCULATOR, "-i"])
    assert snapshot.ok, snapshot.raw
    tree = (snapshot.data or {}).get("tree")
    assert count_name(tree, "7") == 1, "the recovery target must have a unique name"
    ref = _first_ref(tree, name="7", role="button")
    assert ref, "the snapshot allocated no ref"
    stop(app)
    read = lambda: any(member[0] == app.pid for member in members(ctx.tier.runtime_dir))
    assert not read(), "stale-ref setup did not stop Calculator"
    ctx.before("proc.calculator", read())
    envelope = ctx.product(["click", ref])
    ctx.after("proc.calculator", read())
    assert not read(), "stale click relaunched the dead owner"
    assert not envelope.ok, envelope.raw
    assert error_code(envelope) == "STALE_REF", envelope.raw
    error = envelope.error or {}
    details = error.get("details") or {}
    assert details.get("process_state") == "exited", envelope.raw
    assert details.get("retryable") is False, envelope.raw
    assert error.get("disposition") == {"delivery": "not_delivered", "retry": "safe"}, envelope.raw
    recovery = error.get("recovery") or {}
    assert recovery.get("strategy") == "inspect_state_then_retry_original", envelope.raw
    assert recovery.get("requires_fresh_snapshot") is True, envelope.raw
    ctx.restored("proc.calculator", read())


def _first_ref(tree: dict | None, *, name=None, role=None) -> str | None:
    return qualified_ref(tree, name=name, role=role)
