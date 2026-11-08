#!/usr/bin/env python3
"""Model-free privacy gate for the two sheets that leaked non-bench names into model runs:
TextEdit's Save As panel (the textedit-seeded setup, collapsed and expanded) and Finder's
Go to Folder sheet (on open, and after a full bench path is set in its field).

For each state it takes the snapshots a model would take and counts, inside row-like nodes
(cells, rows, tree items, list items, menu items) and their descendants, the texts that are
not bench names. It prints counts only, never a name. Snapshots and refmaps live in a
throwaway AGENT_DESKTOP_HOME that is deleted at the end.

Never presses Return. Leaves 0 windows and 0 sheets. Run it through bench/probe_slot.sh:
  bash bench/probe_slot.sh privacy-gate 600 bench/out/logs/privacy-gate.txt \
      python3 bench/privacy_gate.py <agent-desktop binary>
Exit 0 only when every sheet snapshot shows 0 non-bench row texts. The whole-window `-i`
snapshot is reported beside them but does not gate (the Finder window sidebar is not a sheet).
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
from tasks import (BENCH_DIR, BENCH_FINDER_PREFIXES, PERSONAL_FINDER_TITLES, close_finder_windows,  # noqa: E402
                   macctl, open_bench_finder_window, quit_app, quit_textedit_for_prefs, sh)

SHEETCTL = ROOT / "out" / "sheetctl"
ROW_ROLES = {"cell", "row", "treeitem", "outlinerow", "listitem", "menuitem", "browser", "outline"}
TEXT_KEYS = ("name", "value", "description", "label", "title", "placeholder")
BENCH_MARKERS = tuple(BENCH_FINDER_PREFIXES) + ("seed-", "note-", "t5-", str(BENCH_DIR))
HOME = str(Path.home())


def is_bench_text(text, own_names):
    stripped = text.strip().strip("⁨⁩").strip()
    if not stripped or stripped in own_names:
        return True
    if any(marker in stripped for marker in BENCH_MARKERS):
        return True
    return False


def is_personal_marker(text):
    stripped = text.strip().strip("⁨⁩").strip()
    if stripped in PERSONAL_FINDER_TITLES or "iCloud" in stripped:
        return True
    return HOME in stripped and str(BENCH_DIR) not in stripped


def category(text):
    stripped = text.strip().strip("\u2068\u2069").strip()
    if stripped in PERSONAL_FINDER_TITLES:
        return "personal_folder_title"
    if "iCloud" in stripped:
        return "icloud"
    if HOME in stripped or stripped.startswith("~"):
        return "home_path_outside_bench"
    if "/" in stripped:
        return "other_path"
    return "other_text"


def texts_of(node):
    return [node[key] for key in TEXT_KEYS if isinstance(node.get(key), str) and node.get(key)]


def count_tree(tree, own_names):
    counts = {"row_nodes": 0, "row_texts": 0, "non_bench_row_texts": 0, "personal_marker_texts_anywhere": 0}

    def visit(node, in_row):
        row = in_row or str(node.get("role", "")).lower() in ROW_ROLES
        if row and not in_row:
            counts["row_nodes"] += 1
        for text in texts_of(node):
            if is_personal_marker(text):
                counts["personal_marker_texts_anywhere"] += 1
            if row:
                counts["row_texts"] += 1
                if not is_bench_text(text, own_names):
                    counts["non_bench_row_texts"] += 1
                    key = "nb_" + category(text) + "_" + str(node.get("role", "?")).lower()
                    counts[key] = counts.get(key, 0) + 1
        for child in node.get("children", []) or []:
            visit(child, row)

    visit(tree or {}, False)
    return counts


def snapshot_counts(binary, app, own_names, extra):
    done = subprocess.run([binary, "snapshot", "--app", app, *extra], capture_output=True, text=True, timeout=60)
    try:
        envelope = json.loads(done.stdout)
    except json.JSONDecodeError:
        return {"args": " ".join(extra), "ok": False, "parse": "failed"}
    data = envelope.get("data") or {}
    row = {"args": " ".join(extra), "ok": envelope.get("ok"), "complete": data.get("complete"),
           "ref_count": data.get("ref_count")}
    if not envelope.get("ok"):
        row["error_code"] = (envelope.get("error") or {}).get("code")
    row.update(count_tree(data.get("tree"), own_names))
    return row


SNAPSHOT_VARIANTS = [["--surface", "sheet", "-i"], ["--surface", "sheet"], ["--surface", "sheet", "--max-depth", "30"],
                     ["-i"]]


def measure(binary, app, state, own_names):
    rows = [dict(snapshot_counts(binary, app, own_names, variant), state=state, gating="--surface" in variant)
            for variant in SNAPSHOT_VARIANTS]
    for row in rows:
        print(json.dumps(row), flush=True)
    return rows


def sheet_count(app, title):
    out = subprocess.run([str(SHEETCTL), "present", app, title], capture_output=True, text=True, timeout=20).stdout
    try:
        return json.loads(out).get("sheets", 0)
    except json.JSONDecodeError:
        return -1


def wait_sheets(app, title, want, tries=40):
    for _ in range(tries):
        if sheet_count(app, title) == want:
            return True
        time.sleep(0.25)
    return False


def front_title(app):
    return (json.loads(macctl("windows", app) or "[]") or [""])[0]


def textedit_gate(binary, gate_id):
    folder = BENCH_DIR / f"probe-privacy-{gate_id}"
    src, dest = folder / "src", folder / "dest"
    src.mkdir(parents=True)
    dest.mkdir()
    seed = f"seed-{gate_id}"
    own_names = {"src", "dest", f"{seed}.rtf", seed, folder.name}
    blank = folder / "blank.txt"
    blank.write_text("")
    sh("textutil", "-convert", "rtf", str(blank), "-output", str(src / f"{seed}.rtf"))
    blank.unlink()
    rows = []
    try:
        quit_textedit_for_prefs()
        subprocess.run(["open", "-a", "TextEdit", str(src / f"{seed}.rtf")], check=True, timeout=10)
        title = ""
        for _ in range(50):
            title = str(json.loads(macctl("focused", "TextEdit") or "{}").get("window", ""))
            if title.startswith(seed):
                break
            time.sleep(0.2)
        else:
            raise RuntimeError("seeded document did not open")
        macctl("raise", "TextEdit", title)
        if not front_title("TextEdit").startswith(seed):
            raise RuntimeError("seeded document is not in front; no key sent")
        macctl("key", "TextEdit", "1", "cmd,shift,alt")
        if not wait_sheets("TextEdit", title, 1):
            raise RuntimeError("Save As sheet did not open")
        time.sleep(1.0)
        expanded = json.loads(subprocess.run([str(SHEETCTL), "expanded", "TextEdit", title], capture_output=True,
                                             text=True, timeout=20).stdout or "{}").get("expanded")
        rows += measure(binary, "TextEdit", f"textedit-saveas-{'expanded' if expanded else 'collapsed'}", own_names)
        subprocess.run([str(SHEETCTL), "toggle", "TextEdit", title], capture_output=True, timeout=20)
        time.sleep(1.5)
        expanded2 = json.loads(subprocess.run([str(SHEETCTL), "expanded", "TextEdit", title], capture_output=True,
                                              text=True, timeout=20).stdout or "{}").get("expanded")
        rows += measure(binary, "TextEdit", f"textedit-saveas-{'expanded' if expanded2 else 'collapsed'}-toggled",
                        own_names)
        if expanded2 != expanded:
            subprocess.run([str(SHEETCTL), "toggle", "TextEdit", title], capture_output=True, timeout=20)
            time.sleep(1.0)
        subprocess.run([str(SHEETCTL), "cancel", "TextEdit", title], capture_output=True, timeout=20)
        wait_sheets("TextEdit", title, 0)
    finally:
        quit_app("TextEdit")
        shutil.rmtree(folder, ignore_errors=True)
    return rows


def finder_gate(binary, gate_id):
    folder = BENCH_DIR / f"probe-privacy-{gate_id}-finder"
    folder.mkdir(parents=True)
    own_names = {folder.name}
    rows = []
    try:
        open_bench_finder_window(folder)
        title = folder.name
        if front_title("Finder") != title:
            raise RuntimeError("bench Finder window is not in front; no key sent")
        macctl("key", "Finder", "5", "cmd,shift")
        if not wait_sheets("Finder", title, 1):
            raise RuntimeError("Go to Folder sheet did not open")
        time.sleep(1.0)
        rows += measure(binary, "Finder", "finder-goto-open", own_names)
        focused = json.loads(macctl("focused", "Finder") or "{}")
        if focused.get("role") in ("AXTextField", "AXComboBox") and focused.get("window") == title:
            macctl("set-focused", "Finder", str(folder))
            time.sleep(1.5)
            rows += measure(binary, "Finder", "finder-goto-bench-path-set", own_names)
        else:
            print(json.dumps({"state": "finder-goto-bench-path-set", "skipped": "path field not focused"}))
        if front_title("Finder") == title:
            macctl("key", "Finder", "53")
        if not wait_sheets("Finder", title, 0):
            subprocess.run([str(SHEETCTL), "cancel", "Finder", title], capture_output=True, timeout=20)
            wait_sheets("Finder", title, 0)
    finally:
        close_finder_windows(lambda t: t == folder.name)
        shutil.rmtree(folder, ignore_errors=True)
    return rows


def main():
    binary = os.path.abspath(sys.argv[1])
    gate_id = uuid.uuid4().hex[:6]
    state_home = tempfile.mkdtemp(prefix="privacy-gate-")
    os.environ["AGENT_DESKTOP_HOME"] = state_home
    rows = []
    try:
        for gate in (textedit_gate, finder_gate):
            try:
                rows += gate(binary, gate_id)
            except Exception as error:
                print(json.dumps({"gate": gate.__name__, "error": type(error).__name__, "detail": str(error)[:120]}))
                rows.append({"state": gate.__name__, "ok": False, "non_bench_row_texts": -1})
    finally:
        shutil.rmtree(state_home, ignore_errors=True)
    gating = [r for r in rows if r.get("gating", True)]
    failing = [r for r in gating if r.get("non_bench_row_texts", 0) != 0 or not r.get("ok")]
    states = sorted({r["state"] for r in rows})
    print(json.dumps({"states": states, "measurements": len(rows), "failing_measurements": len(failing),
                      "non_bench_row_texts_total": sum(max(r.get("non_bench_row_texts", 0), 0) for r in gating),
                      "personal_marker_texts_total": sum(r.get("personal_marker_texts_anywhere", 0) for r in rows)}))
    print("GATE PASS" if gating and not failing else "GATE FAIL")
    sys.exit(0 if gating and not failing else 1)


if __name__ == "__main__":
    main()
