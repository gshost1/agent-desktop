"""The planned cells of one pass, saved before the first run: every (round, task, arm) in the order they run.

Each round runs the tasks in a seeded random order. Inside a task's block the arms run in a row of a Latin square
built from a seeded random arm order, and each task enters the square at its own seeded offset; over as many rounds
as there are arms, every arm takes every slot of every task's block exactly once. First-order carry-over (which arm
ran just before) is not balanced: each run starts from a reset desktop and a fresh tool home instead.

    python3 -I schedule.py make <experiment> <arms,csv> <tasks,csv> <rounds> <seed> > schedule.json
    python3 -I schedule.py check schedule.json
    python3 -I schedule.py cells schedule.json              # one "slot round task arm" line per cell, in order
    python3 -I schedule.py pending schedule.json ledger     # the cells not attempted yet
    python3 -I schedule.py matches schedule.json <experiment> <arms,csv>
"""
import json
import random
import sys
from collections import Counter


def make(experiment, arms, tasks, rounds, seed):
    rng = random.Random(seed)
    base = list(arms)
    rng.shuffle(base)
    square = [base[i:] + base[:i] for i in range(len(base))]
    offset = {t: rng.randrange(len(base)) for t in tasks}
    cells = []
    for r in range(rounds):
        order = list(tasks)
        rng.shuffle(order)
        for t in order:
            for pos, arm in enumerate(square[(r + offset[t]) % len(base)]):
                cells.append({"slot": len(cells), "round": r, "task": t, "arm": arm, "position": pos})
    return {"experiment": experiment, "seed": seed, "arms": list(arms), "tasks": list(tasks), "rounds": rounds,
            "cells": cells}


def problems(plan):
    """Why a saved schedule is not the balanced, complete plan it claims to be; [] when it is."""
    out = []
    arms, tasks, rounds, cells = plan["arms"], plan["tasks"], plan["rounds"], plan["cells"]
    want = Counter((r, t, a) for r in range(rounds) for t in tasks for a in arms)
    have = Counter((c["round"], c["task"], c["arm"]) for c in cells)
    if have != want:
        out.append("cells are not exactly one per round, task and arm")
    if [c["slot"] for c in cells] != list(range(len(cells))):
        out.append("slots are not 0..n-1 in order")
    for t in tasks:
        pos = Counter((c["arm"], c["position"]) for c in cells if c["task"] == t)
        counts = [pos.get((a, p), 0) for a in arms for p in range(len(arms))]
        if max(counts) - min(counts) > (0 if rounds % len(arms) == 0 else 1):
            out.append(f"task {t}: arm positions are not balanced")
    return out


def main():
    cmd = sys.argv[1]
    if cmd == "make":
        name, arms, tasks, rounds, seed = sys.argv[2:7]
        print(json.dumps(make(name, arms.split(","), tasks.split(","), int(rounds), int(seed)), indent=1))
        return 0
    plan = json.load(open(sys.argv[2]))
    if cmd == "check":
        bad = problems(plan)
        print("\n".join(bad) or f"ok: {len(plan['cells'])} cells, balanced")
        return 1 if bad else 0
    if cmd == "cells":
        for c in plan["cells"]:
            print(c["slot"], c["round"], c["task"], c["arm"])
        return 0
    if cmd == "matches":
        name, arms = sys.argv[3], sys.argv[4].split(",")
        return 0 if plan["experiment"] == name and sorted(plan["arms"]) == sorted(arms) else 1
    if cmd == "pending":
        done = attempted_slots(plan["experiment"], sys.argv[3])
        for c in plan["cells"]:
            if c["slot"] not in done:
                print(c["slot"], c["round"], c["task"], c["arm"])
        return 0
    return 2


def attempted_slots(experiment, ledger_path):
    """Slots of this experiment with any ledger line. A started attempt with no end state was interrupted: it
    stays a failed attempt and is never run again, so a resumed pass cannot retry a run that went badly."""
    done = set()
    try:
        lines = open(ledger_path).read().splitlines()
    except OSError:
        return done
    for line in lines:
        try:
            r = json.loads(line)
        except ValueError:
            continue
        if r.get("experiment") == experiment and r.get("slot") is not None:
            done.add(int(r["slot"]))
    return done


if __name__ == "__main__":
    sys.exit(main())
