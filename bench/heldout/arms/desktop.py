"""Bench-app hygiene for native runs: close a bench app between runs without touching anything else.

tasks_heldout.close_bench_app refused an app that showed any window without a bench title, so an app
panel a run left open (TextEdit's Fonts or Colors, Preview's Adjust Color) stopped every later setup, and
an app stuck in an open menu ignored quit, so setup failed with "did not quit". This version:
  - treats the app's own panels (PANEL_TITLES, or a title that names a bench document) as the bench's;
  - waits for an app that is busy right after a save to answer the window query;
  - sends Escape to the app twice before quitting, which ends a menu's tracking loop;
  - force-stops the app when quit does not take, but only after re-reading its windows and finding
    nothing but bench documents and panels.
Any other window still stops the run before the model starts, and the app is never touched.

tasks_heldout.py is frozen by sha256, so run_native.py installs this over its close_bench_app
(`desktop.install(th)`); open_bench_document and every task's cleanup call it by that name.
"""
import json
import os
import re
import signal
import subprocess
import time

BENCH_PREFIXES = ("seed-ho-", "bench-ho-")
PANEL_TITLES = {"Fonts", "Colors", "Adjust Color", "Adjust Size", "Inspector", "Spelling and Grammar",
                "Substitutions", "Typography", "Show Colors", "Show Fonts"}
ESCAPE_KEYCODE = "53"
CRASH_HOST = "UserNotificationCenter"
IGNORE_BUTTON = re.compile(r'^\s*(@\S+) button "Ignore"')
CRASH_DIALOGS_MAX = 20


class Desktop:
    """The side effects, injectable for tests: macctl, pids and signals."""

    def __init__(self, macctl, sleep=time.sleep, reader=None, clicker=None):
        self.macctl = macctl
        self.sleep = sleep
        self.reader = reader
        self.clicker = clicker

    def pids(self, app):
        out = subprocess.run(["pgrep", "-x", app], capture_output=True, text=True).stdout
        return [int(p) for p in out.split()]

    def kill(self, pid, sig):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass

    def windows(self, app):
        try:
            titles = json.loads(self.macctl("windows", app) or "null")
        except (json.JSONDecodeError, subprocess.TimeoutExpired):
            return None
        return [str(t) for t in titles] if isinstance(titles, list) else None


TOP_LEVEL = re.compile(r"^ {0,2}(?:@\S+ )?(?:window|dialog|sheet)\b")


def dialog_blocks(lines):
    """The AX lines of each top-level window or dialog (a snapshot may nest them one level under `application`)."""
    blocks = []
    for line in lines:
        if TOP_LEVEL.match(line):
            blocks.append([])
        if blocks:
            blocks[-1].append(line)
    return blocks


def crash_dialogs(d, apps):
    """(app, Ignore ref) for every crash-report dialog that names one of `apps`. Other alerts are read only to test
    for those exact phrases; their text is never returned or printed."""
    if d.reader is None or not d.pids(CRASH_HOST):
        return []
    out = []
    for block in dialog_blocks(d.reader(CRASH_HOST)):
        text = " ".join(block)
        named = [a for a in apps if f"{a} quit unexpectedly" in text]
        refs = [m.group(1) for m in (IGNORE_BUTTON.match(line) for line in block) if m]
        if named and refs:
            out.append((named[0], refs[0]))
    return out


def dismiss_crash_dialogs(d, apps):
    """Click Ignore on each crash-report dialog of a bench app; returns the apps whose dialogs were dismissed."""
    gone = []
    for _ in range(CRASH_DIALOGS_MAX):
        found = crash_dialogs(d, apps)
        if not found:
            break
        app, ref = found[0]
        d.clicker(ref)
        gone.append(app)
        d.sleep(1)
    return gone


def is_bench_window(title):
    return title.startswith(BENCH_PREFIXES) or any(p in title for p in BENCH_PREFIXES)


def is_panel(title):
    return title in PANEL_TITLES


def foreign_windows(titles, allowed=None):
    return [t for t in titles if not (is_bench_window(t) or is_panel(t) or (allowed is not None and allowed(t)))]


def read_windows(d, app, wait_s):
    for _ in range(max(1, int(wait_s / 2))):
        titles = d.windows(app)
        if titles is not None:
            return titles
        if not d.pids(app):
            return []
        d.sleep(2)
    return None


def checked_windows(d, app, wait_s, allowed=None):
    titles = read_windows(d, app, wait_s)
    if titles is None:
        raise RuntimeError(f"{app} does not answer the window query; not touching it")
    foreign = foreign_windows(titles, allowed)
    if foreign:
        raise RuntimeError(f"{app} has a window that is not a bench window open ({len(foreign)}); not touching it")
    return titles


def wait_gone(d, app, seconds):
    for _ in range(int(seconds / 0.2)):
        if not d.pids(app):
            return True
        d.sleep(0.2)
    return not d.pids(app)


def close_app(d, app, wait_s=30, allowed=None, discard=()):
    """Quit `app` when it shows only bench documents, its own panels and windows `allowed(title)` accepts; raise
    otherwise. Windows whose titles start with a bench prefix or one of `discard` are closed without saving."""
    if not d.pids(app):
        return
    checked_windows(d, app, wait_s, allowed)
    for pid in d.pids(app):
        for _ in range(2):
            d.macctl("keypid", str(pid), ESCAPE_KEYCODE)
    titles = checked_windows(d, app, wait_s, allowed)
    panels = sorted({t for t in titles if is_panel(t)})
    if panels:
        d.macctl("close-discard", app, *panels)
    d.macctl("close-discard", app, *BENCH_PREFIXES, *discard, timeout=60)
    d.macctl("quit", app)
    if wait_gone(d, app, 10):
        return
    checked_windows(d, app, wait_s, allowed)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for pid in d.pids(app):
            d.kill(pid, sig)
        if wait_gone(d, app, 3):
            return
    raise RuntimeError(f"{app} did not quit")


def install(th, reader=None, clicker=None):
    """Replace the task set's close_bench_app with close_app on the real desktop. A task set may name the other
    windows its apps may show (`window_allowed(app, title)`) and more title prefixes to discard (DISCARD_PREFIXES).
    reader(app) returns an app's AX lines and clicker(ref) clicks one, for crash-report dialogs."""
    d = Desktop(th.macctl, reader=reader, clicker=clicker)
    allowed_for = getattr(th, "window_allowed", None)
    discard = tuple(getattr(th, "DISCARD_PREFIXES", ()))

    def close(app):
        allowed = (lambda title: allowed_for(app, title)) if allowed_for else None
        close_app(d, app, allowed=allowed, discard=discard)

    th.close_bench_app = close
    return d
