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
import signal
import subprocess
import time

BENCH_PREFIXES = ("seed-ho-", "bench-ho-")
PANEL_TITLES = {"Fonts", "Colors", "Adjust Color", "Adjust Size", "Inspector", "Spelling and Grammar",
                "Substitutions", "Typography", "Show Colors", "Show Fonts"}
ESCAPE_KEYCODE = "53"


class Desktop:
    """The side effects, injectable for tests: macctl, pids and signals."""

    def __init__(self, macctl, sleep=time.sleep):
        self.macctl = macctl
        self.sleep = sleep

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


def is_bench_window(title):
    return title.startswith(BENCH_PREFIXES) or any(p in title for p in BENCH_PREFIXES)


def is_panel(title):
    return title in PANEL_TITLES


def foreign_windows(titles):
    return [t for t in titles if not (is_bench_window(t) or is_panel(t))]


def read_windows(d, app, wait_s):
    for _ in range(max(1, int(wait_s / 2))):
        titles = d.windows(app)
        if titles is not None:
            return titles
        if not d.pids(app):
            return []
        d.sleep(2)
    return None


def checked_windows(d, app, wait_s):
    titles = read_windows(d, app, wait_s)
    if titles is None:
        raise RuntimeError(f"{app} does not answer the window query; not touching it")
    foreign = foreign_windows(titles)
    if foreign:
        raise RuntimeError(f"{app} has a window that is not a bench window open ({len(foreign)}); not touching it")
    return titles


def wait_gone(d, app, seconds):
    for _ in range(int(seconds / 0.2)):
        if not d.pids(app):
            return True
        d.sleep(0.2)
    return not d.pids(app)


def close_app(d, app, wait_s=30):
    """Quit `app` when it shows only bench documents and its own panels; raise otherwise."""
    if not d.pids(app):
        return
    checked_windows(d, app, wait_s)
    for pid in d.pids(app):
        for _ in range(2):
            d.macctl("keypid", str(pid), ESCAPE_KEYCODE)
    titles = checked_windows(d, app, wait_s)
    panels = sorted({t for t in titles if is_panel(t)})
    if panels:
        d.macctl("close-discard", app, *panels)
    d.macctl("close-discard", app, *BENCH_PREFIXES, timeout=60)
    d.macctl("quit", app)
    if wait_gone(d, app, 10):
        return
    checked_windows(d, app, wait_s)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for pid in d.pids(app):
            d.kill(pid, sig)
        if wait_gone(d, app, 3):
            return
    raise RuntimeError(f"{app} did not quit")


def install(th):
    """Replace tasks_heldout.close_bench_app with close_app on the real desktop."""
    d = Desktop(th.macctl)
    th.close_bench_app = lambda app: close_app(d, app)
    return d
