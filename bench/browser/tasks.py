"""Task definitions and independent verifiers for bench/browser/run.py."""
import json
import re
import subprocess
from pathlib import Path

WEB = ["ax", "cdp", "hybrid", "ad-web"]
CROSS = ["ax", "hybrid", "ad-web"]

PERSONAL_FOLDERS_HOLD = ("held: the Save panel and a new Finder window start in the user's personal folders "
                         "(iCloud, Documents, Recents), and no bench may open or read those")
TASKS = [
    {"id": "w1", "channels": WEB,
     "prompt": "In Google Chrome, open {base}/w1/ and tell me the order total shown on the page."},
    {"id": "w2", "channels": WEB,
     "prompt": ("In Google Chrome, open {base}/w2/ and fill in the shipping form: full name 'Ada Lovelace', "
                "email 'ada@example.test', country Norway, Express shipping, gift wrap ticked, notes 'Leave at door'. "
                "Then place the order.")},
    {"id": "w3", "channels": WEB,
     "prompt": ("In Google Chrome, open {base}/w3/ and subscribe: choose the Pro plan, 7 seats, accept the terms "
                "and confirm. Tell me the confirmation code.")},
    {"id": "w4", "channels": WEB,
     "prompt": "In Google Chrome, open {base}/w4/ and tell me how many units of '{exp[w4_name]}' are in stock."},
    {"id": "x1", "channels": CROSS, "kill": ["TextEdit"], "held": PERSONAL_FOLDERS_HOLD,
     "prompt": ("In Google Chrome, open {base}/x1/ and copy the quote text. Put it into a new plain-text TextEdit "
                "document (Format > Make Plain Text) and save it through the Save dialog as {work}/x1.txt .")},
    {"id": "x2", "channels": CROSS, "kill": ["Calculator"],
     "prompt": ("In Google Chrome, open {base}/x2/ , read the units and the unit price, then use the Calculator app "
                "to multiply them. Leave the product on Calculator's display.")},
    {"id": "x3", "channels": CROSS, "kill": ["Finder-noop"], "held": PERSONAL_FOLDERS_HOLD,
     "prompt": ("In Google Chrome, open {base}/x3/ and download the weekly report. Chrome saves downloads to "
                "{downloads} . Then, in Finder, go to that folder and rename the downloaded file to x3-done.csv .")},
]


def _events(state, kind):
    return [e["fields"] for e in state["events"] if e["kind"] == kind]


def _calculator_display(ad):
    out = subprocess.run([ad, "snapshot", "--app", "Calculator"], capture_output=True, text=True, timeout=30).stdout
    try:
        tree = json.loads(out)["data"]["tree"]
    except Exception:
        return ""
    texts = []

    def walk(n):
        for k in ("value", "name"):
            v = n.get(k)
            if isinstance(v, str):
                texts.append(v)
        for c in n.get("children", []):
            walk(c)

    walk(tree)
    return " | ".join(texts)


def verify(task, ctx, answer, state):
    exp = ctx["exp"]
    tid = task["id"]
    if tid == "w1":
        return exp["total"] in answer.replace(",", ""), f"want {exp['total']}"
    if tid == "w2":
        subs = _events(state, "w2")
        want = {"name": "Ada Lovelace", "email": "ada@example.test", "country": "Norway",
                "shipping": "express", "gift": "yes", "notes": "Leave at door"}
        good = any(all(s.get(k) == v for k, v in want.items()) for s in subs)
        return good, f"{len(subs)} submissions; last={subs[-1] if subs else None}"
    if tid == "w3":
        subs = _events(state, "w3")
        good = any(s.get("plan") == "Pro" and s.get("seats") == "7" and s.get("terms") == "yes" for s in subs)
        return good and exp["code"] in answer, f"{len(subs)} submissions; code_in_answer={exp['code'] in answer}"
    if tid == "w4":
        return re.search(rf"\b{exp['w4_stock']}\b", answer) is not None, f"want {exp['w4_stock']}"
    if tid == "x1":
        p = Path(ctx["work"]) / "x1.txt"
        text = p.read_text(errors="ignore") if p.exists() else ""
        norm = lambda s: re.sub(r"\s+", " ", s.replace("’", "'")).strip()
        return norm(exp["quote"]) in norm(text), f"exists={p.exists()} bytes={len(text)}"
    if tid == "x2":
        product = exp["x2_a"] * exp["x2_b"]
        shown = _calculator_display(ctx["ad"])
        return re.search(rf"(?<![\d.]){product}(?![\d.])", shown.replace(",", "").replace(" ", "")) is not None, \
            f"want {product}; display~{shown[:120]}"
    if tid == "x3":
        d = Path(ctx["downloads"])
        names = sorted(x.name for x in d.iterdir()) if d.exists() else []
        return "x3-done.csv" in names and exp["report"] not in names, f"files={names}"
    return False, "unknown task"
