"""Stray-save check: a run fails if it leaves a fresh file whose name starts with ':' in its work folder.

A Save panel given a full path as the file name saves `:private:tmp:...` into its default folder and the
tool still reports success. Only folders the bench created are read: the user's personal folders (Documents,
Desktop, Downloads, iCloud, Pictures and the rest) are never opened or listed. Strays are moved (never
deleted) into the run's results folder.
"""
import shutil
from pathlib import Path


def collect_strays(since, work, dest):
    found = []
    for p in Path(work).rglob(":*"):
        try:
            if p.is_file() and p.lstat().st_mtime >= since:
                found.append(p)
        except OSError:
            continue
    moved = []
    for p in found:
        Path(dest).mkdir(parents=True, exist_ok=True)
        shutil.move(str(p), str(Path(dest) / p.name))
        moved.append(f"{p.parent.name}/{p.name[-60:]}")
    return moved
