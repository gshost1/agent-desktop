#!/usr/bin/env python3
"""Post-probe window hygiene check. A probe must leave 0 bench windows, 0 sheets on them and 0
personal-folder windows. Prints counts only, never a title.

Usage: python3 bench/post_probe_check.py <probe name> [--clean]
  exit 0: clean. exit 1: something was left (with --clean it is then closed, and the probe
  still counts as failed, because it left it behind).
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from tasks import (BENCH_FINDER_PREFIXES, PERSONAL_CLOSE_LOG, PERSONAL_FINDER_TITLES,  # noqa: E402
                   close_finder_windows, macctl)

TEXTEDIT_BENCH_MARKERS = ("bench", "probe", "seed.", "note-")


def finder_counts():
    titles = json.loads(macctl("windows", "Finder") or "[]")
    bench = sum(1 for t in titles if t.startswith(tuple(BENCH_FINDER_PREFIXES)))
    personal = sum(1 for t in titles if t in PERSONAL_FINDER_TITLES)
    return bench, personal


def textedit_bench_windows():
    titles = json.loads(macctl("windows", "TextEdit") or "[]")
    return [t for t in titles if any(marker in t for marker in TEXTEDIT_BENCH_MARKERS)]


def on_screen_finder_windows():
    return sum(1 for w in json.loads(macctl("owners") or "[]") if w.get("owner") == "Finder" and w.get("layer") == 0)


def main():
    name = sys.argv[1]
    clean = "--clean" in sys.argv
    bench, personal = finder_counts()
    textedit = textedit_bench_windows()
    row = {"probe": name, "finder_bench_windows": bench, "finder_personal_windows": personal,
           "textedit_bench_windows": len(textedit), "finder_on_screen_layer0": on_screen_finder_windows()}
    dirty = bench or personal or textedit
    if dirty and clean:
        closed_bench, left_bench = close_finder_windows(lambda t: t.startswith(tuple(BENCH_FINDER_PREFIXES)))
        closed_personal, left_personal = close_finder_windows(lambda t: t in PERSONAL_FINDER_TITLES)
        for title in textedit:
            macctl("close-discard", "TextEdit", title)
        row.update({"closed_bench": closed_bench, "closed_personal": closed_personal,
                    "left_after_clean": left_bench + left_personal + len(textedit_bench_windows())})
        if closed_personal or left_personal:
            with open(PERSONAL_CLOSE_LOG, "a") as log:
                log.write(f"post-probe {name} closed={closed_personal} left={left_personal}\n")
    row["verdict"] = "FAIL: probe left windows behind" if dirty else "clean"
    print(json.dumps(row))
    sys.exit(1 if dirty else 0)


if __name__ == "__main__":
    main()
