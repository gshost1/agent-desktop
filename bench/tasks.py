"""The five benchmark tasks: setup, prompt and independent verification.

Verification never trusts the agent's own `ok`: it reads the file system, the
OS (`sw_vers`, `sysctl`) or the app's accessibility tree directly after the run.
Task 1's app is not named by the public benchmark; Weather is used here.
"""
import json
import re
import shutil
import subprocess
import time
from pathlib import Path

BENCH_DIR = Path.home() / "agent-desktop-bench"
APPS = ["Weather", "System Settings", "TextEdit", "Calculator"]


def sh(*args, timeout=30):
    return subprocess.run(list(args), capture_output=True, text=True, timeout=timeout).stdout


MACCTL = Path(__file__).resolve().parent / "out" / "macctl"
BENCH_DOC_PREFIXES = ["Untitled", "note-", "sp-", "seed-", str(BENCH_DIR)]
BENCH_FINDER_PREFIXES = ["t3-", "bench-", "agent-desktop-bench", "probe-", "savepath-"]


def build_macctl():
    source = MACCTL.parent.parent / "macctl.swift"
    if not MACCTL.exists() or MACCTL.stat().st_mtime < source.stat().st_mtime:
        MACCTL.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["swiftc", "-O", str(source), "-o", str(MACCTL)], check=True, capture_output=True)


def macctl(*args, timeout=20):
    """Bench control through AppKit and Accessibility only: no AppleScript, no Automation grant."""
    build_macctl()
    return subprocess.run([str(MACCTL), *args], capture_output=True, text=True, timeout=timeout).stdout


def quit_app(name):
    if name == "TextEdit":
        macctl("close-discard", "TextEdit", *BENCH_DOC_PREFIXES, timeout=60)
    macctl("quit", name)


TEXTEDIT_PREFS = Path.home() / "Library/Containers/com.apple.TextEdit/Data/Library/Preferences/com.apple.TextEdit"
SAVE_FOLDER_KEY = "NSNavLastRootDirectory"
SAVE_FOLDER_ORIGINAL = Path(__file__).resolve().parent / "out" / "textedit-save-folder-original.json"
SAVE_FOLDER_LOG = Path(__file__).resolve().parent / "out" / "textedit-save-folder.log"


def log_save_folder(event, **fields):
    with open(SAVE_FOLDER_LOG, "a") as fh:
        fh.write(json.dumps({"t": time.strftime("%Y-%m-%d %H:%M:%S"), "event": event, **fields}) + "\n")


def read_save_folder():
    """TextEdit's last save folder from its container prefs, as (present, value). Reads only that key."""
    done = subprocess.run(["defaults", "read", str(TEXTEDIT_PREFS), SAVE_FOLDER_KEY],
                          capture_output=True, text=True, timeout=10)
    return (True, done.stdout.strip()) if done.returncode == 0 else (False, None)


def textedit_gone(tries=50):
    for _ in range(tries):
        if subprocess.run(["pgrep", "-x", "TextEdit"], capture_output=True).returncode != 0:
            return True
        time.sleep(0.2)
    return False


def textedit_holds_only_bench_windows():
    try:
        titles = json.loads(macctl("windows", "TextEdit") or "[]")
    except (json.JSONDecodeError, subprocess.TimeoutExpired):
        return False
    return all(any(t.startswith(p) for p in BENCH_DOC_PREFIXES) for t in titles)


def quit_textedit_for_prefs():
    """Quit TextEdit before its prefs are written. A TextEdit wedged on a bench
    document (a stuck Save sheet blocks a normal quit) is force-killed, but only
    when every window it has is a bench window; anything else is left alone."""
    quit_app("TextEdit")
    if textedit_gone():
        return
    if textedit_holds_only_bench_windows():
        subprocess.run(["pkill", "-9", "-x", "TextEdit"], timeout=10)
        if textedit_gone(25):
            return
    raise RuntimeError("TextEdit is still running; its save-folder pref was not touched")


def restore_save_folder():
    """Put back the save folder recorded by point_save_folder, if one is pending. Safe to call any time."""
    if not SAVE_FOLDER_ORIGINAL.exists():
        return
    original = json.loads(SAVE_FOLDER_ORIGINAL.read_text())
    quit_textedit_for_prefs()
    if original["present"]:
        subprocess.run(["defaults", "write", str(TEXTEDIT_PREFS), SAVE_FOLDER_KEY, "-string", original["value"]],
                       check=True, timeout=10)
    else:
        subprocess.run(["defaults", "delete", str(TEXTEDIT_PREFS), SAVE_FOLDER_KEY],
                       capture_output=True, timeout=10)
    now = read_save_folder()
    restored = now == (original["present"], original["value"])
    log_save_folder("restore", original=original, now={"present": now[0], "value": now[1]}, restored=restored)
    if not restored:
        raise RuntimeError(f"TextEdit save folder not restored: now {now}, original {original}")
    SAVE_FOLDER_ORIGINAL.unlink()


def point_save_folder(folder):
    """Make TextEdit's Save panel open in the run's bench folder, so it never shows
    iCloud, Documents or any other folder of George's. Records the original first."""
    if BENCH_DIR not in Path(folder).resolve().parents:
        raise ValueError(f"refusing to point TextEdit's save folder outside {BENCH_DIR}: {folder}")
    restore_save_folder()
    quit_textedit_for_prefs()
    present, value = read_save_folder()
    SAVE_FOLDER_ORIGINAL.write_text(json.dumps({"present": present, "value": value}))
    subprocess.run(["defaults", "write", str(TEXTEDIT_PREFS), SAVE_FOLDER_KEY, "-string", str(folder)],
                   check=True, timeout=10)
    now = read_save_folder()
    log_save_folder("point", old={"present": present, "value": value}, new=str(folder), now=now[1])
    if now != (True, str(folder)):
        restore_save_folder()
        raise RuntimeError(f"TextEdit save folder did not take: {now}")


def open_bench_finder_window(folder):
    """Open the task's own bench folder in a front Finder window, so the run never
    starts on Finder's default window (Recents, Home or a personal folder)."""
    if BENCH_DIR not in Path(folder).resolve().parents:
        raise ValueError(f"refusing to open a Finder window outside {BENCH_DIR}: {folder}")
    clear_finder_alert()
    subprocess.run(["open", str(folder)], check=True, timeout=10)
    time.sleep(1.5)
    macctl("raise", "Finder", Path(folder).name)


FINDER_ALERT = MACCTL.parent / "finder_alert"


def clear_finder_alert():
    """A Finder alert (one was left by a failed rename on 2026-11-08) blocks every new
    window, so `open <bench dir>` silently opens nothing. Dismiss an OK-only alert;
    stop on any other dialog."""
    source = MACCTL.parent.parent / "finder_alert.swift"
    if not FINDER_ALERT.exists() or FINDER_ALERT.stat().st_mtime < source.stat().st_mtime:
        subprocess.run(["swiftc", "-O", str(source), "-o", str(FINDER_ALERT)], check=True, capture_output=True)
    done = subprocess.run([str(FINDER_ALERT)], capture_output=True, text=True, timeout=20)
    if done.stdout.strip():
        print(done.stdout.strip(), flush=True)
    if done.returncode == 2:
        raise RuntimeError("a Finder dialog other than an OK alert is open; not touching it")


def close_bench_finder_windows():
    macctl("close-discard", "Finder", *BENCH_FINDER_PREFIXES)


def reset_desktop():
    for app in APPS:
        quit_app(app)
    clear_finder_alert()
    close_bench_finder_windows()


def walk(node):
    yield node
    for child in node.get("children", []) or []:
        yield from walk(child)


def snapshot_tree(real_bin, app):
    out = sh(real_bin, "snapshot", "--app", app)
    try:
        return json.loads(out).get("data", {}).get("tree") or {}
    except ValueError:
        return {}


def norm(text):
    return re.sub(r"\s+", " ", (text or "").lower())


class Task:
    key = ""
    app = ""

    def __init__(self, run_id, real_bin):
        self.run_id = run_id
        self.real_bin = real_bin

    def setup(self):
        pass

    def prompt(self):
        raise NotImplementedError

    def verify(self, answer):
        raise NotImplementedError

    def cleanup(self):
        pass


class WeatherCard(Task):
    key = "weather"
    app = "Weather"

    def prompt(self):
        return ("Open the Weather app and read the first location card in its location list. "
                "Reply in the form: LOCATION | TEMPERATURE | CONDITIONS.")

    def verify(self, answer):
        tree = snapshot_tree(self.real_bin, "Weather")
        cards = []
        for node in walk(tree):
            if node.get("name") == "Location List":
                cards = [n.get("name", "") for n in walk(node) if n.get("role") == "button" and "°" in n.get("name", "")]
                break
        if not cards:
            return False, "could not read Weather location list after the run"
        first = cards[0]
        location = first.split(",")[0].strip()
        temp = re.search(r"-?\d+°", first)
        ok = norm(location) in norm(answer) and temp is not None and temp.group(0) in answer
        return ok, f"expected first card {first!r}"


class SettingsAbout(Task):
    key = "settings"
    app = "System Settings"

    def prompt(self):
        return ("Open System Settings, go to General > About, and report the macOS version "
                "and the chip shown there. Reply in the form: VERSION | CHIP.")

    def verify(self, answer):
        version = sh("sw_vers", "-productVersion").strip()
        chip = sh("sysctl", "-n", "machdep.cpu.brand_string").strip()
        ok = version in answer and norm(chip) in norm(answer)
        return ok, f"expected {version} and {chip}"


class FinderFolder(Task):
    key = "finder"
    app = "Finder"

    def setup(self):
        self.parent = BENCH_DIR / f"t3-{self.run_id}"
        if self.parent.exists():
            shutil.rmtree(self.parent)
        self.parent.mkdir(parents=True)
        self.name = f"bench-{self.run_id}"
        open_bench_finder_window(self.parent)

    def prompt(self):
        return (f"Using Finder, create a new folder named `{self.name}` inside the folder "
                f"`{self.parent}`. Reply DONE when the folder exists.")

    def verify(self, answer):
        entries = sorted(p.name for p in self.parent.iterdir() if p.name != ".DS_Store")
        ok = entries == [self.name] and (self.parent / self.name).is_dir()
        return ok, f"folder contents {entries}"

    def cleanup(self):
        close_bench_finder_windows()
        shutil.rmtree(self.parent, ignore_errors=True)


class TextEditSave(Task):
    key = "textedit"
    app = "TextEdit"

    def setup(self):
        self.folder = BENCH_DIR / f"t4-{self.run_id}"
        if self.folder.exists():
            shutil.rmtree(self.folder)
        self.folder.mkdir(parents=True)
        point_save_folder(self.folder)
        self.stem = f"note-{self.run_id}"
        self.text = f"hello from run {self.run_id}"
        self.default_dirs = [Path.home() / "Library/Mobile Documents/com~apple~TextEdit/Documents",
                             Path.home() / "Documents", Path.home() / "Desktop"]

    def prompt(self):
        return (f"Open TextEdit, create a new document containing exactly the text `{self.text}`, "
                f"and save it through the Save dialog with the name `{self.stem}` in the folder "
                f"`{self.folder}`. Reply DONE when it is saved.")

    def verify(self, answer):
        strays = [f"{p.parent.name}/{p.name}" for p in self.stray_paths()]
        if strays:
            return False, f"stray files outside the target folder {strays}"
        entries = sorted(p.name for p in self.folder.iterdir() if p.name != ".DS_Store")
        if len(entries) != 1 or Path(entries[0]).stem != self.stem:
            return False, f"folder contents {entries}"
        body = sh("textutil", "-convert", "txt", "-stdout", str(self.folder / entries[0]))
        ok = body.strip() == self.text
        return ok, f"file {entries[0]} body {body.strip()[:80]!r}"

    def cleanup(self):
        quit_app("TextEdit")
        restore_save_folder()
        shutil.rmtree(self.folder, ignore_errors=True)
        quarantine = Path(__file__).resolve().parent / "out" / "icloud-strays"
        for path in self.stray_paths():
            quarantine.mkdir(parents=True, exist_ok=True)
            shutil.move(str(path), str(quarantine / path.name))

    def stray_paths(self):
        """This run's own file names in TextEdit's default folders, checked by exact
        path. The folders themselves are never listed: they hold George's files."""
        names = [self.stem + ext for ext in ("", ".rtf", ".rtfd", ".txt", ".html", ".docx", ".odt")]
        return [d / n for d in self.default_dirs for n in names if (d / n).exists()]


class TextEditSeeded(Task):
    """TextEdit Save into a given folder, starting from a document the bench seeded
    and opened inside the run's bench folder, so Save As starts there and the panel
    never shows TextEdit's default folder. Not comparable with the Aside rows."""
    key = "textedit-seeded"
    app = "TextEdit"

    def setup(self):
        self.folder = BENCH_DIR / f"t5-{self.run_id}"
        if self.folder.exists():
            shutil.rmtree(self.folder)
        self.src, self.dest = self.folder / "src", self.folder / "dest"
        self.src.mkdir(parents=True)
        self.dest.mkdir()
        self.seed = f"seed-{self.run_id}"
        self.stem = f"note-{self.run_id}"
        self.text = f"hello from run {self.run_id}"
        self.default_dirs = [Path.home() / "Library/Mobile Documents/com~apple~TextEdit/Documents",
                             Path.home() / "Documents", Path.home() / "Desktop"]
        blank = self.folder / "blank.txt"
        blank.write_text("")
        sh("textutil", "-convert", "rtf", str(blank), "-output", str(self.src / f"{self.seed}.rtf"))
        blank.unlink()
        quit_textedit_for_prefs()
        subprocess.run(["open", "-a", "TextEdit", str(self.src / f"{self.seed}.rtf")], check=True, timeout=10)
        for _ in range(50):
            focused = json.loads(macctl("focused", "TextEdit") or "{}")
            if str(focused.get("window", "")).startswith(self.seed):
                break
            time.sleep(0.2)
        else:
            raise RuntimeError("the seeded TextEdit document did not open")

    def prompt(self):
        return (f"TextEdit has the document `{self.seed}.rtf` open. Type exactly the text `{self.text}` into it, "
                f"then save it with File > Save As… under the name `{self.stem}` in the folder `{self.dest}`. "
                f"Reply DONE when it is saved.")

    def verify(self, answer):
        strays = [f"{p.parent.name}/{p.name}" for p in self.stray_paths()]
        if strays:
            return False, f"stray files outside the target folder {strays}"
        entries = sorted(p.name for p in self.dest.iterdir() if p.name != ".DS_Store")
        if len(entries) != 1 or Path(entries[0]).stem != self.stem:
            return False, f"dest contents {entries}"
        body = sh("textutil", "-convert", "txt", "-stdout", str(self.dest / entries[0]))
        ok = body.strip() == self.text
        return ok, f"file {entries[0]} body {body.strip()[:80]!r}"

    def cleanup(self):
        quit_app("TextEdit")
        shutil.rmtree(self.folder, ignore_errors=True)
        quarantine = Path(__file__).resolve().parent / "out" / "stray-saves"
        for path in self.stray_paths():
            quarantine.mkdir(parents=True, exist_ok=True)
            shutil.move(str(path), str(quarantine / path.name))

    def stray_paths(self):
        """This run's own file names in TextEdit's default folders and in src, checked
        by exact path. The default folders are never listed: they hold George's files."""
        names = [self.stem + ext for ext in ("", ".rtf", ".rtfd", ".txt", ".html", ".docx", ".odt")]
        dirs = self.default_dirs + [self.src]
        return [d / n for d in dirs for n in names if (d / n).exists()]


class CalculatorMultiply(Task):
    key = "calculator"
    app = "Calculator"

    def prompt(self):
        return ("Use the Calculator app to compute 1234 × 5678 and report the result shown "
                "in its display. Reply with the number only.")

    def verify(self, answer):
        tree = snapshot_tree(self.real_bin, "Calculator")
        display = ""
        for node in walk(tree):
            if node.get("role") == "scrollarea" and node.get("name") == "Edit field":
                texts = [n.get("value") or n.get("name") or "" for n in walk(node) if n.get("role") == "statictext"]
                display = texts[0] if texts else ""
        digits = re.sub(r"\D", "", display)
        ok = digits == "7006652" and "7006652" in re.sub(r"\D", "", answer)
        return ok, f"display {display!r}"


TASKS = {t.key: t for t in [WeatherCard, SettingsAbout, FinderFolder, TextEditSave, TextEditSeeded, CalculatorMultiply]}

FORBIDDEN = {"open", "osascript", "mkdir", "touch", "cp", "mv", "defaults", "sw_vers", "sysctl",
             "system_profiler", "textutil", "screencapture", "shortcuts", "bc", "python", "python3",
             "ls", "cat", "find", "stat", "mdfind", "rm", "say", "uname"}


def audit_command(cmd):
    """Return the first forbidden program a Bash command runs as a command word, or None."""
    for segment in re.split(r"&&|\|\||[;|\n`]|\$\(|\(", cmd):
        words = [w for w in segment.strip().split() if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", w)]
        if words and words[0].split("/")[-1] in FORBIDDEN:
            return words[0]
    return None
