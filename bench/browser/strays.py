"""Stray-save check: a run fails if it leaves files in the iCloud TextEdit folder or names starting with ':'.

A Save panel given a full path as the file name saves `:private:tmp:...` into its default folder and the
tool still reports success. Strays are moved (never deleted) into the run's results folder.
"""
import shutil
import subprocess
from pathlib import Path

ICLOUD_TEXTEDIT = Path.home() / "Library/Mobile Documents/com~apple~TextEdit/Documents"
WATCHED = [ICLOUD_TEXTEDIT, Path.home() / "Documents", Path.home() / "Desktop", Path.home() / "Downloads"]


def _list(folder):
    try:
        out = subprocess.run(["ls", "-A", str(folder)], capture_output=True, text=True, timeout=10).stdout
    except Exception:
        return []
    return [folder / n for n in out.splitlines() if n]


def collect_strays(since, work, dest):
    found = []
    for folder in WATCHED + [Path(work)]:
        for p in _list(folder):
            try:
                fresh = p.lstat().st_mtime >= since
            except OSError:
                continue
            if not fresh:
                continue
            if folder == ICLOUD_TEXTEDIT or p.name.startswith(":"):
                found.append(p)
    moved = []
    for p in found:
        Path(dest).mkdir(parents=True, exist_ok=True)
        target = Path(dest) / p.name
        shutil.move(str(p), str(target))
        moved.append(f"{p.parent.name}/{p.name[-60:]}")
    return moved
