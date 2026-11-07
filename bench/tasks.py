"""The five benchmark tasks: setup, prompt and independent verification.

Verification never trusts the agent's own `ok`: it reads the file system, the
OS (`sw_vers`, `sysctl`) or the app's accessibility tree directly after the run.
Task 1's app is not named by the public benchmark; Weather is used here.
"""
import json
import re
import shutil
import subprocess
from pathlib import Path

BENCH_DIR = Path.home() / "agent-desktop-bench"
APPS = ["Weather", "System Settings", "TextEdit", "Calculator"]


def sh(*args, timeout=30):
    return subprocess.run(list(args), capture_output=True, text=True, timeout=timeout).stdout


def quit_app(name):
    script = f'if application "{name}" is running then tell application "{name}" to quit saving no'
    if name != "TextEdit":
        script = f'if application "{name}" is running then tell application "{name}" to quit'
    subprocess.run(["osascript", "-e", script], capture_output=True, timeout=20)


def close_bench_finder_windows():
    script = ('tell application "Finder" to close (every Finder window whose name starts with "t3-" '
              'or name starts with "bench-" or name is "agent-desktop-bench")')
    subprocess.run(["osascript", "-e", script], capture_output=True, timeout=20)


def reset_desktop():
    for app in APPS:
        quit_app(app)
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
        self.stem = f"note-{self.run_id}"
        self.text = f"hello from run {self.run_id}"

    def prompt(self):
        return (f"Open TextEdit, create a new document containing exactly the text `{self.text}`, "
                f"and save it through the Save dialog with the name `{self.stem}` in the folder "
                f"`{self.folder}`. Reply DONE when it is saved.")

    def verify(self, answer):
        entries = sorted(p.name for p in self.folder.iterdir() if p.name != ".DS_Store")
        if len(entries) != 1 or Path(entries[0]).stem != self.stem:
            return False, f"folder contents {entries}"
        body = sh("textutil", "-convert", "txt", "-stdout", str(self.folder / entries[0]))
        ok = body.strip() == self.text
        return ok, f"file {entries[0]} body {body.strip()[:80]!r}"

    def cleanup(self):
        quit_app("TextEdit")
        shutil.rmtree(self.folder, ignore_errors=True)


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


CHROME_PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>Invoice {inv}</title></head>
<body><header><nav><a href="#">Home</a> <a href="#">Orders</a> <a href="#">Support</a></nav></header>
<main><h1>Invoice {inv}</h1><table><thead><tr><th>Item</th><th>Qty</th><th>Price</th></tr></thead><tbody>
{rows}</tbody></table><p>Shipping: $7.50</p><p><strong>Order total:</strong> <span id="total">${total}</span></p>
<button>Pay now</button> <button>Download PDF</button></main></body></html>"""


class ChromeRead(Task):
    """Read a value from a page already open in a real Chrome (throwaway profile)."""
    key = "chrome"
    app = "Google Chrome"

    def setup(self):
        import random
        if subprocess.run(["pgrep", "-x", "Google Chrome"], capture_output=True).returncode == 0:
            raise RuntimeError("Google Chrome is already running; refusing to touch a user browser")
        rnd = random.Random(self.run_id)
        self.total = f"{rnd.randint(100, 9999)}.{rnd.randint(10, 99)}"
        rows = "".join(f"<tr><td>Item {i}</td><td>{rnd.randint(1, 5)}</td><td>${rnd.randint(5, 300)}.00</td></tr>\n"
                       for i in range(12))
        self.dir = BENCH_DIR / f"chrome-{self.run_id}"
        self.dir.mkdir(parents=True, exist_ok=True)
        page = self.dir / "invoice.html"
        page.write_text(CHROME_PAGE.format(inv=self.run_id[-6:], rows=rows, total=self.total))
        self.profile = self.dir / "profile"
        subprocess.run(["open", "-na", "Google Chrome", "--args", f"--user-data-dir={self.profile}",
                        "--no-first-run", "--no-default-browser-check", page.as_uri()])
        import time
        for _ in range(40):
            out = sh(self.real_bin, "list-windows", "--app", "Google Chrome")
            if "Invoice" in out:
                break
            time.sleep(0.5)
        time.sleep(1)

    def prompt(self):
        return ("Google Chrome has a page open that shows an invoice. Read the order total from that page. "
                "Reply with the amount only.")

    def verify(self, answer):
        ok = self.total in answer.replace(",", "")
        return ok, f"expected {self.total}"

    def cleanup(self):
        subprocess.run(["pkill", "-f", f"user-data-dir={self.profile}"], capture_output=True)
        import time
        time.sleep(2)
        shutil.rmtree(self.dir, ignore_errors=True)


TASKS = {t.key: t for t in [WeatherCard, SettingsAbout, FinderFolder, TextEditSave, CalculatorMultiply, ChromeRead]}

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
