#!/usr/bin/env python3
"""Live Mac checks for the laptop's draft PRs #18 (wait --menu-closed) and #14
(capture verdicts). Run under the desktop lock (owner "mac-verify").

Uses only folders this script creates under ~/agent-desktop-bench/ and the repo's
own fixture app. Every ref it acts on comes from a snapshot of the bench window
it opened itself, found by its exact title; it stops if that window is missing.

Usage: python3 bench/mac_verify.py pr18|pr14 <agent-desktop binary> [fixture.app]
"""
import json
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
from tasks import BENCH_DIR, macctl, open_bench_finder_window  # noqa: E402

LOCK = Path.home() / ".agent-desktop" / "bench.lock"


def run(binary, *args, timeout=60):
    t0 = time.time()
    done = subprocess.run([binary, *args], capture_output=True, text=True, timeout=timeout)
    try:
        body = json.loads(done.stdout)
    except json.JSONDecodeError:
        body = {"raw": done.stdout[-400:], "stderr": done.stderr[-400:]}
    return body, round(time.time() - t0, 2)


def walk(node):
    yield node
    for child in node.get("children") or []:
        yield from walk(child)


def bench_folder(kind, files):
    folder = BENCH_DIR / f"bench-{kind}-{uuid.uuid4().hex[:6]}"
    folder.mkdir(parents=True)
    for name in files:
        (folder / name).touch()
    return folder


def bench_window(binary, folder):
    body, _ = run(binary, "list-windows", "--app", "Finder")
    data = body.get("data") or []
    windows = data if isinstance(data, list) else data.get("windows", [])
    wins = [w for w in windows if w.get("title") == folder.name]
    if len(wins) != 1:
        sys.exit(f"STOP: expected one Finder window titled {folder.name}, found {len(wins)}")
    return wins[0]["id"]


def close_folder(folder):
    macctl("close-discard", "Finder", folder.name)
    if BENCH_DIR in folder.resolve().parents:
        shutil.rmtree(folder, ignore_errors=True)


def summary(body):
    err = body.get("error") or {}
    data = body.get("data") or {}
    return {"ok": body.get("ok"), "code": err.get("code"), "details": err.get("details"),
            "recovery": err.get("recovery"), "disposition": err.get("disposition"),
            "data": {k: data[k] for k in ("state", "menu", "closed", "elapsed_ms", "satisfied") if k in data}}


def verify_pr18(binary):
    results = []
    folder = bench_folder("menu", ["a.txt", "b.txt", "c.txt"])
    try:
        open_bench_finder_window(folder)
        wid = bench_window(binary, folder)
        for attempt in (1, 2):
            snap, _ = run(binary, "snapshot", "--app", "Finder", "--window-id", wid)
            tree = (snap.get("data") or {}).get("tree") or {}
            target = next((n for n in walk(tree) if n.get("name") == "a.txt" and n.get("ref_id")), None)
            if not target:
                sys.exit("STOP: no a.txt ref in the bench window snapshot")
            opened, _ = run(binary, "right-click", target["ref_id"])
            menu, menu_s = run(binary, "wait", "--menu", "--app", "Finder", "--timeout", "3000")
            t0 = time.time()
            waiter = subprocess.Popen([binary, "wait", "--menu-closed", "--app", "Finder", "--timeout", "15000"],
                                      stdout=subprocess.PIPE, text=True)
            time.sleep(3)
            escape_at = round(time.time() - t0, 2)
            macctl("key", "Finder", "53")
            out, _ = waiter.communicate(timeout=30)
            returned_at = round(time.time() - t0, 2)
            results.append({"case": f"menu open, attempt {attempt}", "right_click": summary(opened),
                            "menu_seen": summary(menu), "menu_wait_s": menu_s, "escape_at_s": escape_at,
                            "wait_returned_at_s": returned_at, "wait": summary(json.loads(out or "{}"))})
            time.sleep(1)
        for attempt in (1, 2):
            body, took = run(binary, "wait", "--menu-closed", "--app", "Finder", "--timeout", "15000")
            results.append({"case": f"no menu, attempt {attempt}", "wait_s": took, "wait": summary(body)})
    finally:
        close_folder(folder)
    return results


def verify_pr14(binary, fixture):
    results = []
    folder = bench_folder("large", [f"f{i:04d}.txt" for i in range(3000)])
    try:
        open_bench_finder_window(folder)
        time.sleep(2)
        wid = bench_window(binary, folder)
        for budget in ("200", "100", "50", "20"):
            snap, took = run(binary, "snapshot", "--app", "Finder", "--window-id", wid, "--timeout-ms", budget)
            data = snap.get("data") or {}
            refs = [n for n in walk(data.get("tree") or {}) if n.get("ref_id") and n.get("name", "").startswith("f")]
            row = {"case": f"snapshot --timeout-ms {budget}", "s": took, "complete": data.get("complete"),
                   "truncated": data.get("truncated"), "nodes_observed": data.get("nodes_observed"),
                   "file_refs": len(refs)}
            if data.get("complete") is False and refs:
                acted, _ = run(binary, "click", refs[0]["ref_id"])
                row["click_first_file_ref"] = summary(acted)
                results.append(row)
                break
            results.append(row)
    finally:
        close_folder(folder)
    subprocess.run(["open", "-n", str(fixture)], check=True, timeout=10)
    try:
        time.sleep(3)
        snap, _ = run(binary, "snapshot", "--app", "AgentDeskFixture")
        tree = (snap.get("data") or {}).get("tree") or {}
        twins = [n for n in walk(tree) if n.get("name") == "twin-control" and n.get("ref_id")]
        status = next((n.get("value") for n in walk(tree) if n.get("name") == "twin-status"), None)
        row = {"case": "fixture twin", "complete": (snap.get("data") or {}).get("complete"),
               "twin_refs": len(twins), "status_before": status}
        if twins:
            acted, _ = run(binary, "click", twins[0]["ref_id"])
            row["click_twin"] = summary(acted)
            after, _ = run(binary, "snapshot", "--app", "AgentDeskFixture")
            row["status_after"] = next((n.get("value") for n in walk((after.get("data") or {}).get("tree") or {})
                                        if n.get("name") == "twin-status"), None)
        results.append(row)
    finally:
        subprocess.run(["pkill", "-x", "AgentDeskFixture"], timeout=10)
    return results


def main():
    if not (LOCK.exists() and LOCK.read_text().startswith("mac-verify")):
        sys.exit(f"take the desktop lock as mac-verify first: {LOCK}")
    which, binary = sys.argv[1], sys.argv[2]
    results = verify_pr18(binary) if which == "pr18" else verify_pr14(binary, sys.argv[3])
    out = ROOT / "out" / f"mac-verify-{which}.json"
    out.write_text(json.dumps(results, indent=1))
    print(json.dumps(results, indent=1))


if __name__ == "__main__":
    main()
