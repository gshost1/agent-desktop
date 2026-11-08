"""Task definitions and independent verifiers for bench/browser/run.py."""
import json
import re
import subprocess
import urllib.request
from pathlib import Path

WEB = ["ax", "cdp", "hybrid", "ad-web"]
CROSS = ["ax", "hybrid", "ad-web"]
REAL = ["cdp", "ad-web"]
BENCH_UA = "agent-desktop-bench/1.0"
REAL_RULES = (" This is a public website. Do not log in, do not submit or send anything, and decline or dismiss "
              "any cookie or consent banner.")
WIKI_SENTENCE = ("Python was conceived in the late 1980s by Guido van Rossum at Centrum Wiskunde & Informatica (CWI) "
                 "in the Netherlands.")
COPYTREE_SIGNATURE = ("shutil.copytree(src, dst, symlinks=False, ignore=None, copy_function=copy2, "
                      "ignore_dangling_symlinks=False, dirs_exist_ok=False)")
FORM_WANT = {"custname": "Ada Lovelace", "custtel": "555-0142", "custemail": "ada@example.test", "size": "medium",
             "topping": ["bacon", "mushroom"], "delivery": "19:30", "comments": "Ring the bell twice"}

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

REAL_TASKS = [
    {"id": "r1", "channels": REAL,
     "prompt": ("In Google Chrome, open https://en.wikipedia.org/wiki/Python_(programming_language) and tell me "
                "the first sentence of its 'History' section, word for word." + REAL_RULES)},
    {"id": "r2", "channels": REAL,
     "prompt": ("In Google Chrome, open https://github.com/rust-lang/rust and tell me the latest release tag and "
                "the star count shown on the repository page." + REAL_RULES)},
    {"id": "r3", "channels": REAL,
     "prompt": ("In Google Chrome, open https://www.weather.gov/ , look up the forecast for Honolulu, HI using the "
                "site's own location search, and tell me today's forecast high temperature in degrees F." + REAL_RULES)},
    {"id": "r4", "channels": REAL,
     "prompt": ("In Google Chrome, open https://docs.python.org/3/ , use the site's search box to search for "
                "'shutil.copytree', open the result for the function shutil.copytree, and tell me its full "
                "signature." + REAL_RULES)},
    {"id": "r5", "channels": REAL,
     "prompt": ("In Google Chrome, open https://httpbin.org/forms/post and fill every field: customer name "
                "'Ada Lovelace', telephone '555-0142', e-mail 'ada@example.test', pizza size Medium, toppings "
                "Bacon and Mushroom, delivery time 19:30, delivery instructions 'Ring the bell twice'. "
                "Check that every field holds the value. Do NOT press the submit button." + REAL_RULES)},
]
TASKS.extend(REAL_TASKS)


def _get_json(url):
    request = urllib.request.Request(url, headers={"User-Agent": BENCH_UA})
    with urllib.request.urlopen(request, timeout=20) as r:
        return json.loads(r.read())


def real_expectations(task_id):
    """Values that change over time are read from the public API just before the run; fixed ones are in the module."""
    if task_id == "r2":
        tag = _get_json("https://api.github.com/repos/rust-lang/rust/releases/latest")["tag_name"]
        stars = _get_json("https://api.github.com/repos/rust-lang/rust")["stargazers_count"]
        return {"r2_tag": tag, "r2_stars": stars}
    if task_id == "r3":
        periods = _get_json("https://api.weather.gov/gridpoints/HFO/154,145/forecast")["properties"]["periods"]
        today = next(p for p in periods if p["isDaytime"])
        return {"r3_high": today["temperature"], "r3_name": today["name"]}
    return {}


def _page_form(ctx):
    script = Path(__file__).resolve().parent / "cdp_form.mjs"
    out = subprocess.run(["node", str(script), str(ctx["cdp_port"]), "httpbin.org"], capture_output=True, text=True,
                         timeout=30)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"error": (out.stderr or out.stdout)[:200]}


def _stars_match(answer, stars):
    for number, suffix in re.findall(r"(\d[\d,]*(?:\.\d+)?)\s*([kK])?", answer):
        value = float(number.replace(",", "")) * (1000 if suffix else 1)
        if abs(value - stars) <= max(1, stars * 0.015):
            return True
    return False


def _verify_real(tid, ctx, answer):
    exp = ctx["exp"]
    squash = lambda s: re.sub(r"\s+", " ", s.replace("\u2019", "'")).strip()
    if tid == "r1":
        return squash(WIKI_SENTENCE) in squash(answer) and len(answer) < 400, "sentence match"
    if tid == "r2":
        tag_ok = exp["r2_tag"] in answer
        return tag_ok and _stars_match(answer, exp["r2_stars"]), f"want tag {exp['r2_tag']} stars~{exp['r2_stars']}"
    if tid == "r3":
        nums = [int(n) for n in re.findall(r"(?<![\d.])(\d{2,3})(?![\d])", answer)]
        return any(abs(n - exp["r3_high"]) <= 1 for n in nums), f"want {exp['r3_high']} ({exp['r3_name']})"
    if tid == "r4":
        compact = lambda s: re.sub(r"\s+", "", s)
        return compact(COPYTREE_SIGNATURE) in compact(answer), "signature match"
    if tid == "r5":
        page = _page_form(ctx)
        if "error" in page:
            return False, f"form read failed: {page['error']}"
        fields = page.get("fields", {})
        still_form = page.get("path") == "/forms/post"
        good = all(fields.get(k) == v for k, v in FORM_WANT.items())
        return good and still_form, f"path={page.get('path')} fields={fields}"
    return False, "unknown real task"


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
    if tid.startswith("r"):
        return _verify_real(tid, ctx, answer)
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
