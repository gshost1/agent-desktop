#!/usr/bin/env python3
"""Re-run the forbidden-command audit over recorded native rows and list every row whose verdict changes.

Each row keeps the Bash commands the model ran (steps[].tools[].cmd). The recorded verdict came from the
audit that ran with the row; the new one comes from common.audit_native (shellwords). A row the old audit
failed only for a forbidden command is re-judged from its verifier message: it passes when that message is
one the same task's passing rows carry.

Usage: python3 -I heldout/reaudit.py <results-native.jsonl> [--home <bench HOME the rows ran under>]
       [--write <fixed jsonl>]
"""
import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "arms"))
import common  # noqa: E402
import tasks_heldout as th  # noqa: E402

FORBIDDEN_PREFIX = re.compile(r"^forbidden command (\S+); ")


def commands_of(row):
    return [t["cmd"] for s in row.get("steps") or [] for t in s.get("tools") or [] if t.get("cmd")]


def new_audit(row, skills):
    for cmd in commands_of(row):
        hit = common.audit_native(cmd, row["arm"], th.FORBIDDEN, skills)
        if hit:
            return hit, cmd
    return None, None


def passing_messages(rows):
    out = defaultdict(set)
    for r in rows:
        if r.get("success"):
            out[r["task"]].add(r.get("verify", ""))
    return out


def reaudit(rows, skills):
    """Rows with a changed verdict, as (row, old_hit, new_hit, cmd, new_success, new_verify)."""
    good = passing_messages(rows)
    changes = []
    for r in rows:
        if r.get("suite") != "native":
            continue
        why = r.get("verify", "")
        m = FORBIDDEN_PREFIX.match(why)
        old_hit = m.group(1) if m else None
        verifier_why = why[m.end():] if m else why
        hit, cmd = new_audit(r, skills)
        if (old_hit is None) == (hit is None):
            continue
        if hit:
            changes.append((r, old_hit, hit, cmd, False, f"forbidden command {hit}; {verifier_why}"))
        else:
            capped = r.get("timed_out")
            ok = verifier_why in good[r["task"]] and not capped
            changes.append((r, old_hit, None, None, ok, verifier_why))
    return changes


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("results")
    ap.add_argument("--home", help="the HOME the rows ran under; its heldout-tools/skills is the guide root")
    ap.add_argument("--write", help="write the rows with re-audited verdicts to this jsonl")
    a = ap.parse_args()
    rows = [json.loads(line) for line in Path(a.results).read_text().splitlines() if line.strip()]
    skills = Path(a.home) / ".agent-desktop-bench-target" / "heldout-tools" / "skills" if a.home else None
    changes = reaudit(rows, skills)
    native = sum(r.get("suite") == "native" for r in rows)
    old_fails = sum(bool(FORBIDDEN_PREFIX.match(r.get("verify", ""))) for r in rows)
    print(f"{native} native rows; {old_fails} failed the old audit; {len(changes)} audit verdicts change")
    print("| arm | task | round | run | old audit | new audit | success old -> new | command |")
    print("|---|---|---|---|---|---|---|---|")
    for r, old_hit, hit, cmd, ok, _ in changes:
        shown = (cmd or "").replace("|", "\\|").replace("\n", " ")[:90]
        print(f"| {r['arm']} | {r['task']} | {r.get('round')} | {r.get('run_id')} | {old_hit or 'clean'} | "
              f"{hit or 'clean'} | {r.get('success')} -> {ok} | `{shown}` |")
    if a.write:
        fixed = {id(r): (ok, why, hit) for r, _, hit, _, ok, why in changes}
        with open(a.write, "w") as fh:
            for r in rows:
                if id(r) in fixed:
                    ok, why, hit = fixed[id(r)]
                    r = dict(r, success=ok, verify=why, reaudit=f"forbidden {hit}" if hit else "clean")
                fh.write(json.dumps(r) + "\n")


if __name__ == "__main__":
    main()
