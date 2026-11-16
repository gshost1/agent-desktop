"""What a run did to the shared desktop and how its process ends, measured instead of read from its commands.

FrontWatch samples the frontmost app while the model runs. A background-only run whose task app comes to the front
broke the rule whatever route did it (a menu call that activates, a launch, a focus call). Only bench app names are
kept; any other frontmost app is recorded as "other", never by name. It also reads the seconds since the last
keyboard and mouse event in the session, which a run that posts global input resets; those are recorded, not
enforced, because a person at the Mac resets them too.

Child runs `claude -p` in its own process group, drains stderr while stdout streams, and on the cap kills the whole
group, so tools the model started cannot outlive the attempt.
"""
import ctypes
import ctypes.util
import os
import signal
import subprocess
import threading
import time

INPUT_TYPES = {"key": 10, "left_click": 1, "right_click": 3, "mouse_move": 5, "scroll": 22}
COMBINED_SESSION = 0


def _seconds_since():
    """Seconds since the last event of each input type in the login session, or {} where it cannot be read."""
    try:
        cg = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))
        f = cg.CGEventSourceSecondsSinceLastEventType
        f.restype, f.argtypes = ctypes.c_double, [ctypes.c_int32, ctypes.c_uint32]
        return {k: f(COMBINED_SESSION, t) for k, t in INPUT_TYPES.items()}
    except (OSError, AttributeError):
        return {}


def front_app():
    """The frontmost app's name, or None when it cannot be read."""
    try:
        asn = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True, timeout=5).stdout.strip()
        if not asn:
            return None
        out = subprocess.run(["lsappinfo", "info", "-only", "name", asn], capture_output=True, text=True,
                             timeout=5).stdout
    except (OSError, subprocess.TimeoutExpired):
        return None
    head, _, value = out.partition("=")
    return value.strip().strip('"') if head.strip().strip('"') == "LSDisplayName" else None


class FrontWatch:
    """Samples the frontmost app every `interval` seconds between start() and stop()."""

    def __init__(self, apps, interval=0.2, reader=front_app, clock=time.time, idle=_seconds_since):
        self.apps = set(apps)
        self.interval = interval
        self.reader = reader
        self.clock = clock
        self.idle = idle
        self.samples = 0
        self.unreadable = 0
        self.fronted = []
        self.start_front = None
        self._last = None
        self._stop = threading.Event()
        self._thread = None
        self._t0 = None

    def label(self, name):
        if name is None:
            return None
        return name if name in self.apps else "other"

    def sample(self):
        name = self.label(self.reader())
        self.samples += 1
        if name is None:
            self.unreadable += 1
        elif name != "other" and name != self._last:
            self.fronted.append((round(self.clock() - self._t0, 2), name))
        if name is not None:
            self._last = name
        return name

    def start(self):
        self._t0 = self.clock()
        self.start_front = self._last = self.label(self.reader())
        self._thread = threading.Thread(target=self._loop, daemon=True)
        self._thread.start()
        return self.start_front

    def _loop(self):
        while not self._stop.wait(self.interval):
            self.sample()

    def stop(self):
        """The record for the row: how the front looked at start, which bench apps came forward, input ages."""
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=5)
        elapsed = self.clock() - self._t0
        ages = self.idle()
        return {"front_at_start": self.start_front, "fronted": self.fronted, "front_samples": self.samples,
                "front_unreadable": self.unreadable,
                "global_input": sorted(k for k, s in ages.items() if s < elapsed) if ages else None}


def foreground_violation(record, allow_foreground):
    """Why a run broke the background-only rule by what it did, or None."""
    if allow_foreground or not record.get("fronted"):
        return None
    t, app = record["fronted"][0]
    return f"foreground: {app} came to the front at {t} s"


class Child:
    """`claude -p` in its own process group with stderr drained on a thread."""

    def __init__(self, cmd, cwd, env):
        self.proc = subprocess.Popen(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     stdin=subprocess.DEVNULL, text=True, start_new_session=True)
        self.stderr = []
        self._drain = threading.Thread(target=self._read_stderr, daemon=True)
        self._drain.start()

    def _read_stderr(self):
        for line in self.proc.stderr:
            self.stderr.append(line)

    def kill_group(self):
        """SIGKILL the whole process group: the CLI, its shells and every tool they started."""
        try:
            os.killpg(self.proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass

    def finish(self):
        """Wait for the CLI, then reap any group member still alive. Returns the stderr text."""
        self.proc.wait()
        self.kill_group()
        self._drain.join(timeout=10)
        return "".join(self.stderr)
