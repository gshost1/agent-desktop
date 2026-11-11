"""Held-out tasks: 10 web, 8 native macOS. Frozen before any run (sha256 in TASKS_FROZEN.md).

Every task is adapted from a public benchmark task, cited in SOURCES. Every verifier reads
state on its own (the bench server's recorded state, the saved file, the system setting, or
the public page fetched by the verifier), never the agent's claim alone.

Web tasks plug into feat-unified-browser/bench/browser/run.py through heldout/arms/run_web.py.
The local site is heldout/web/server.py; its data comes from `world(seed)` below, so the
expected values and the pages come from one source.

Native tasks plug into the bench driver through heldout/arms/run_native.py. They touch only
files the task creates under ~/agent-desktop-bench/bench-ho-*, opened by explicit path, or
read-only System Settings panes whose values the verifier checks are unchanged.
"""
import calendar
import datetime
import html
import importlib.util
import json
import random
import re
import shutil
import subprocess
import sys
import time
import urllib.request
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
BENCH = HERE.parent

BENCH_UA = "agent-desktop-bench/1.0"
PUBLIC_RULES = (" This is a public website. Do not log in, do not submit or send anything, and decline or dismiss "
                "any cookie or consent banner.")
WEB_CHANNELS = ["ax", "cdp", "ad-web"]

SOURCES = {
    "hw1": ("WebArena", "21", "shopping", "List out reviewers, if exist, who mention about ear cups being small"),
    "hw2": ("WebArena", "47", "shopping", "Today is 6/12/2023. Tell me how many fulfilled orders I have over the past "
                                          "month, and the total amount of money I spent."),
    "hw3": ("WebArena", "777", "shopping_admin", "Reduce the price of green Hollister backyard sweater in all size by $5"),
    "hw4": ("WebArena", "404", "reddit", "Upvote the newest post in books subreddit"),
    "hw5": ("WebArena", "409", "reddit", "Reply to the manager of the website in this post with \"thanks! I am a big "
                                         "fan of your website.\""),
    "hw6": ("WebArena", "658", "gitlab", "Create an issue in a11yproject repo with title \"401 bad gateway\". Assign the "
                                         "issue to Roshanjossey. Set due date to be the end of 2030"),
    "hw7": ("WebArena", "269", "shopping", "Show me products under $25 in \"women shoes\" category"),
    "hp1": ("WebVoyager", "ArXiv--17", "arxiv.org", "Find the paper 'GPT-4 Technical Report', when was v3 submitted?"),
    "hp2": ("WebVoyager", "ArXiv--27", "arxiv.org", "On ArXiv, what categories does Economics include, and what are "
                                                    "their abbreviations?"),
    "hp3": ("WebVoyager", "Cambridge Dictionary--17", "dictionary.cambridge.org",
            "How many meanings of \"unblemished\" are given in Cambridge Dictionary? Please browse the page and give "
            "the number directly."),
    "hn1": ("OSWorld", "0b17a146-2934-46c7-8727-73ff6b6483e8", "libreoffice_writer",
            "Help me change the 2 in \"H2O\" to a subscript."),
    "hn2": ("OSWorld", "72b810ef-4156-4d09-8f08-a0cf57e7cefe", "libreoffice_writer",
            "I think the last paragraph is redundant so I want to add strike-through on words in the last paragraph."),
    "hn3": ("OSWorld", "3ef2b351-8a84-4ff2-8724-d86eae9b842e", "libreoffice_writer",
            "Help me center align the heading in LibreOffice."),
    "hn4": ("OSWorld", "e528b65e-1107-4b8c-8988-490e4fece599", "libreoffice_writer",
            "Please help me make the first letter of each word to uppercase."),
    "hn5": ("OSWorld", "72f83cdc-bf76-4531-9a1b-eb893a13f8aa", "gimp", "Please rotate my figure to mirror it horizontally."),
    "hn6": ("OSWorld", "d16c99dc-2a1e-46f2-b350-d97c86c85c15", "gimp",
            "Could you assist me with resizing the dog layer of an image? I need to adjust the height to 512 pixels "
            "while maintaining the original aspect ratio?"),
    "hn7": ("OSWorld", "b6781586-6346-41cd-935a-a6b1487918fc", "os", "I want to set my current time zone to UTC+0."),
    "hn8": ("OSWorld", "bedcedc4-4d72-425e-ad62-21960b11fe0d", "os",
            "Could you set the 'Dim screen when inactive' to off in setting?"),
}

FIRST = ["Ada", "Bram", "Chiara", "Dmitri", "Elena", "Farid", "Greta", "Hiro", "Ines", "Jonas", "Kalani", "Lucia",
         "Mateo", "Nadia", "Oskar", "Priya", "Quentin", "Rosa", "Soren", "Tamsin", "Umar", "Vera", "Wim", "Ximena",
         "Yusuf", "Zofia", "Anton", "Beatrix", "Cyrus", "Delphine"]
LAST = ["Abara", "Brandt", "Castell", "Dorsey", "Ekwueme", "Falk", "Gustafsson", "Halloran", "Ishikawa", "Jovanovic",
        "Kowalczyk", "Lindqvist", "Moreau", "Nakamura", "Okafor", "Petrov", "Quinlan", "Rasmussen", "Salazar",
        "Takahashi", "Uribe", "Varga", "Whitfield", "Xu", "Yilmaz", "Zielinski"]
MONTHS = list(calendar.month_name)[1:]


def fmt_money(value):
    return f"${value:,.2f}"


def long_date(day):
    return f"{MONTHS[day.month - 1]} {day.day}, {day.year}"


def people(rng, count):
    pool = [f"{f} {l}" for f in FIRST for l in LAST]
    return rng.sample(pool, count)


def username(full):
    first, last = full.split(" ", 1)
    return (first[0] + last).lower()


def month_before(day):
    month = day.month - 1 or 12
    year = day.year - (1 if day.month == 1 else 0)
    return day.replace(year=year, month=month, day=min(day.day, calendar.monthrange(year, month)[1]))


EARCUP_COMPLAINTS = [
    "The ear cups are too small for my ears.",
    "Sound is great, but the earcups are a bit small and press on my ears after an hour.",
    "Ear cups feel tiny. My ears do not fit inside them.",
    "I wish the cups were bigger; they are small and get uncomfortable on long flights.",
    "Clamping is fine but the ear cups are small, so they sit on my ears instead of around them.",
]
EARCUP_DISTRACTORS = [
    "The ear cups are soft and roomy, very comfortable.",
    "Small carrying case, but the sound is great.",
    "The headband is a little tight at first.",
    "Battery life is small compared to my old pair.",
    "Big ear cups, deep bass, I wear them all day.",
    "My ears are small and these still fit perfectly.",
]
NEUTRAL_REVIEWS = [
    "Good value for the price.", "Pairing took a minute but works fine now.", "Bass is punchy, mids are clear.",
    "Arrived a day early, well packed.", "The noise cancelling works on the train.", "Microphone is okay for calls.",
    "Folds flat and fits in my bag.", "Controls on the right cup are easy to learn.", "Would buy again.",
    "Sound leaks a little at full volume.", "Charging is fast over USB-C.", "Nice matte finish.",
]
SHOE_WORDS = ["Canvas Sneaker", "Ballet Flat", "Ankle Boot", "Running Shoe", "Loafer", "Espadrille", "Slide Sandal",
              "Mary Jane", "Chelsea Boot", "Trail Runner", "Clog", "Platform Sandal", "Slip-On", "Mule", "Wedge"]
SHOE_BRANDS = ["Arden", "Bellwether", "Corsa", "Dunmore", "Elara", "Fenwick", "Galloway", "Harrow"]
FAMILIES = ["Hollister Backyard Sweater", "Teton Pullover Hoodie", "Circe Hooded Ice Fleece", "Phoebe Zipper Sweatshirt"]
COLORS = ["Green", "Blue", "Red", "Gray"]
SIZES = ["XS", "S", "M", "L", "XL"]
OTHER_PRODUCTS = ["Antonia Racer Tank", "Cronus Yoga Pant", "Taurus Elements Shell", "Juno Jacket", "Ida Workout Parachute Pant",
                  "Hero Hoodie", "Proteus Fitness Jackshirt", "Mars HeatTech Pullover", "Aero Daily Fitness Tee",
                  "Selene Yoga Hoodie", "Electra Bra Top", "Diana Tights", "Orion Two-Tone Fitted Jacket", "Lando Gym Jacket"]
BOOK_TITLES = ["Finished the Earthsea cycle, what next?", "Is it worth rereading Middlemarch?", "Short novels you finished in a day",
               "The library sale haul thread", "Translations of The Master and Margarita compared", "Book club pick for March",
               "Audiobooks that beat the print version", "Underrated science fiction from the 70s",
               "What are you reading this week?", "Favourite opening lines", "Poetry collections for beginners",
               "Long series worth the time", "Non-fiction that reads like a thriller", "Books set in winter",
               "Second-hand bookshops in Lisbon", "Stopped halfway: guilt or relief?"]
OTHER_FORUMS = {"movies": ["Rewatch thread: 90s thrillers", "Best scores of the decade", "Trailer just dropped"],
                "space": ["Launch window tonight", "Telescope buying guide", "Photos of the lunar eclipse"],
                "pics": ["Morning fog over the harbour", "My grandmother's garden", "Abandoned railway bridge"]}
PROJECTS = [("a11yproject", "a11yproject.com"), ("primer", "design"), ("byteblaze", "cloud-to-butt"),
            ("byteblaze", "dotfiles"), ("kkroening", "ffmpeg-python"), ("umano", "AndroidSlidingUpPanel")]


def world(seed):
    """Every value a local web task shows or expects, from one seed."""
    rng = random.Random(f"heldout-web:{seed}")
    names = people(rng, 70)
    w = {"seed": seed}

    product = rng.choice(["Aurora", "Nimbus", "Solace", "Vertex"]) + " Over-Ear Headphones"
    k = rng.randint(3, 5)
    bodies = ([(b, True) for b in rng.sample(EARCUP_COMPLAINTS, k)] + [(b, False) for b in EARCUP_DISTRACTORS]
              + [(b, False) for b in rng.sample(NEUTRAL_REVIEWS, len(NEUTRAL_REVIEWS))])
    rng.shuffle(bodies)
    reviewers = names[:len(bodies)]
    reviews = [{"author": a, "rating": rng.randint(2, 5) if hit else rng.randint(3, 5), "body": b, "hit": hit}
               for a, (b, hit) in zip(reviewers, bodies)]
    w["hw1"] = {"product": product, "reviews": reviews, "expected": sorted(r["author"] for r in reviews if r["hit"]),
                "others": sorted(r["author"] for r in reviews if not r["hit"])}

    today = datetime.date(rng.choice([2024, 2025]), rng.randint(3, 11), rng.randint(5, 25))
    start = month_before(today)
    dates = {start, start - datetime.timedelta(days=1), today}
    while len(dates) < 28:
        dates.add(today - datetime.timedelta(days=rng.randint(1, 150)))
    orders = []
    for i, day in enumerate(sorted(dates)):
        status = rng.choices(["Complete", "Canceled", "Pending", "Processing"], [55, 15, 15, 15])[0]
        if day in (start, start - datetime.timedelta(days=1)):
            status = "Complete"
        orders.append({"number": f"000000{140 + i:03d}", "date": day.isoformat(), "status": status,
                       "total": round(rng.uniform(12, 480), 2), "ship_to": "Emma Lopez"})
    orders.reverse()
    hits = [o for o in orders if o["status"] == "Complete" and start <= datetime.date.fromisoformat(o["date"]) <= today]
    w["hw2"] = {"today": today.isoformat(), "start": start.isoformat(), "orders": orders,
                "count": len(hits), "total": round(sum(o["total"] for o in hits), 2)}

    family, color = rng.choice(FAMILIES), rng.choice(COLORS)
    colors = [color] + rng.sample([c for c in COLORS if c != color], 2)
    products, ids = [], rng.sample(range(1000, 9999), 15 + len(OTHER_PRODUCTS) * 4)
    for c in colors:
        base = rng.randint(40, 70)
        for j, size in enumerate(SIZES):
            products.append({"id": ids.pop(), "name": f"{family}-{size}-{c}", "sku": f"{family[:2].upper()}{j}{c[:2].upper()}",
                             "price": float(base + (2 if size in ("L", "XL") else 0)), "qty": rng.randint(0, 200)})
    for name in OTHER_PRODUCTS:
        for c in rng.sample(COLORS, 2):
            for size in rng.sample(SIZES, 2):
                products.append({"id": ids.pop(), "name": f"{name}-{size}-{c}", "sku": f"{name[:3].upper()}{size}{c[:1]}",
                                 "price": float(rng.randint(18, 90)), "qty": rng.randint(0, 200)})
    rng.shuffle(products)
    targets = {p["id"]: round(p["price"] - 5, 2) for p in products if p["name"].startswith(family) and p["name"].endswith("-" + color)}
    w["hw3"] = {"family": family, "color": color, "products": products, "targets": targets}

    now = datetime.datetime(2025, 6, 1, 12, 0) + datetime.timedelta(hours=rng.randint(0, 2000))
    posts = []
    for i, title in enumerate(rng.sample(BOOK_TITLES, 14)):
        age_h = (i + 1) * rng.randint(5, 9)
        posts.append({"id": rng.randint(10000, 99999), "forum": "books", "title": title, "author": rng.choice(names),
                      "created": (now - datetime.timedelta(hours=age_h)).isoformat(timespec="minutes"),
                      "score": rng.randint(1, 3) if i == 0 else rng.randint(8, 140), "comments": rng.randint(0, 60)})
    for forum, titles in OTHER_FORUMS.items():
        for title in titles:
            posts.append({"id": rng.randint(10000, 99999), "forum": forum, "title": title, "author": rng.choice(names),
                          "created": (now - datetime.timedelta(minutes=rng.randint(5, 600))).isoformat(timespec="minutes"),
                          "score": rng.randint(1, 200), "comments": rng.randint(0, 60)})
    newest = max((p for p in posts if p["forum"] == "books"), key=lambda p: p["created"])
    w["hw4"] = {"posts": posts, "now": now.isoformat(timespec="minutes"), "target": newest["id"]}

    thread_users = rng.sample(names, 9)
    mod = thread_users[0]
    pid = rng.randint(100000, 999999)
    cids = rng.sample(range(1000, 9999), 10)
    comments = [
        {"id": cids[0], "parent": None, "author": thread_users[1], "body": "Does anyone know when the upload limit goes up?"},
        {"id": cids[1], "parent": cids[0], "author": mod, "mod": True,
         "body": "Hi all, I run this site. The upload limit doubles next month, and thank you for all the photos."},
        {"id": cids[2], "parent": cids[1], "author": thread_users[2], "body": "That is great news, thanks!"},
        {"id": cids[3], "parent": None, "author": thread_users[3],
         "body": "Quoting the site manager above: \"the upload limit doubles next month\". Finally."},
        {"id": cids[4], "parent": cids[3], "author": thread_users[4], "body": "About time."},
        {"id": cids[5], "parent": None, "author": thread_users[5], "body": "Lovely shot of the harbour."},
        {"id": cids[6], "parent": cids[5], "author": thread_users[6], "body": "Agreed, the fog makes it."},
        {"id": cids[7], "parent": None, "author": thread_users[7], "body": "Which camera was this taken with?"},
        {"id": cids[8], "parent": cids[7], "author": thread_users[8], "body": "Looks like a phone to me."},
    ]
    tail = comments[5:]
    rng.shuffle(tail)
    comments = comments[:5] + tail
    w["hw5"] = {"forum": "pics", "post": {"id": pid, "title": "Morning fog over the harbour, shot at 6 am",
                                          "author": thread_users[5]},
                "comments": comments, "mod": mod, "mod_comment": cids[1],
                "reply": "thanks! I am a big fan of your website."}

    members = [{"name": n, "username": username(n)} for n in rng.sample(names, 16)]
    assignee = rng.choice(members)
    year, month = rng.randint(2027, 2031), rng.randint(1, 12)
    due = datetime.date(year, month, calendar.monthrange(year, month)[1])
    w["hw6"] = {"members": members, "assignee": assignee, "due_text": f"{MONTHS[month - 1]} {year}",
                "due": due.isoformat(), "group": "a11yproject", "project": "a11yproject.com",
                "labels": ["bug", "help wanted", "documentation", "enhancement", "good first issue"]}

    shoes = []
    for i in range(34):
        price = round(rng.uniform(9, 89), 2)
        if abs(price - 25) < 0.6:
            price = round(price + 1.5, 2)
        shoes.append({"id": 20000 + i, "name": f"{rng.choice(SHOE_BRANDS)} {rng.choice(SHOE_WORDS)} {i:02d}",
                      "category": "women-shoes", "price": price})
    for i in range(7):
        shoes[i]["price"] = round(rng.uniform(9.5, 24.4), 2)
    for i, (cat, base) in enumerate([("men-shoes", 30), ("women-tops", 12), ("girls-shoes", 8)]):
        for j in range(12):
            shoes.append({"id": 30000 + 100 * i + j, "name": f"{rng.choice(SHOE_BRANDS)} {cat.split('-')[1].title()} {j:02d}",
                          "category": cat, "price": round(base + rng.uniform(0, 50), 2)})
    rng.shuffle(shoes)
    w["hw7"] = {"products": shoes, "expected": sorted(p["id"] for p in shoes if p["category"] == "women-shoes" and p["price"] < 25)}
    return w


CATEGORY_NAMES = {"women-shoes": "Women Shoes", "men-shoes": "Men Shoes", "women-tops": "Women Tops",
                  "girls-shoes": "Girls Shoes", "headphones": "Headphones"}


def listing_filter(products, slug, query):
    """The products a /shop/category/<slug> listing shows across all its pages for a query dict."""
    def bound(key):
        try:
            return float(str(query.get(key, "")).replace("$", "").replace(",", "")) if query.get(key) not in (None, "") else None
        except ValueError:
            return None
    lo, hi = bound("price_min"), bound("price_max")
    out = [p for p in products if p["category"] == slug
           and (lo is None or p["price"] >= lo) and (hi is None or p["price"] <= hi)]
    return sorted(out, key=lambda p: (p["price"], p["id"]) if query.get("sort") == "price" else (p["id"],))


def web_seed(task_id, run_idx):
    """Same seed for every arm on a given task and run index, so arms see the same data."""
    return 7000 + 101 * run_idx + sum(map(ord, task_id))


def _norm(text):
    return re.sub(r"\s+", " ", (text or "").replace("’", "'").replace("“", '"').replace("”", '"')).strip()


def _money_values(text):
    return [float(m.replace(",", "")) for m in re.findall(r"(?<![\d.])(\d{1,3}(?:,\d{3})+(?:\.\d+)?|\d+(?:\.\d+)?)", text)]


def verify_hw1(w, answer, state):
    exp = w["hw1"]
    found = [n for n in exp["expected"] if n.lower() in answer.lower()]
    wrong = [n for n in exp["others"] if n.lower() in answer.lower()]
    return len(found) == len(exp["expected"]) and not wrong, f"want {exp['expected']}; missing {sorted(set(exp['expected']) - set(found))}; wrong {wrong}"


def verify_hw2(w, answer, state):
    exp = w["hw2"]
    parts = answer.split("|")
    if len(parts) < 2:
        return False, f"want COUNT | TOTAL = {exp['count']} | {exp['total']:.2f}; answer has no '|'"
    count_ok = re.search(rf"(?<![\d.]){exp['count']}(?![\d.])", parts[0]) is not None
    total_ok = any(abs(v - exp["total"]) < 0.005 for v in _money_values(parts[1]))
    return count_ok and total_ok, f"want {exp['count']} | {exp['total']:.2f}"


def verify_hw3(w, answer, state):
    exp = w["hw3"]
    prices = {int(k): v for k, v in (state.get("admin_prices") or {}).items()}
    bad_targets = {pid: prices.get(pid) for pid, want in exp["targets"].items() if abs(prices.get(pid, -1) - want) > 0.004}
    original = {p["id"]: p["price"] for p in exp["products"]}
    collateral = [pid for pid, price in prices.items() if pid not in exp["targets"] and abs(price - original.get(pid, price)) > 0.004]
    return not bad_targets and not collateral, f"{len(exp['targets'])} targets, wrong {bad_targets}, other products changed {collateral}"


def verify_hw4(w, answer, state):
    votes = {int(k): v for k, v in (state.get("votes") or {}).items() if v}
    target = w["hw4"]["target"]
    return votes == {target: 1}, f"want only post {target} upvoted; votes {votes}"


def verify_hw5(w, answer, state):
    exp = w["hw5"]
    new = state.get("comments") or []
    good = [c for c in new if int(c.get("parent") or 0) == exp["mod_comment"] and _norm(c.get("body")) == _norm(exp["reply"])]
    return len(good) == 1 and len(new) == 1, f"want one reply under comment {exp['mod_comment']}; new comments {new}"


def verify_hw6(w, answer, state):
    exp = w["hw6"]
    issues = [i for i in (state.get("issues") or []) if i.get("project") == f"{exp['group']}/{exp['project']}"]
    good = [i for i in issues if _norm(i.get("title")) == "401 bad gateway" and i.get("assignee") == exp["assignee"]["username"]
            and i.get("due") == exp["due"]]
    return len(good) == 1 and len(issues) == 1, f"want assignee {exp['assignee']['username']} due {exp['due']}; issues {issues}"


def verify_hw7(w, answer, state):
    exp = w["hw7"]
    shop = [n for n in (state.get("navs") or []) if n.get("path", "").startswith("/shop/")]
    if not shop:
        return False, "no shop page was opened"
    last = shop[-1]
    match = re.fullmatch(r"/shop/category/([a-z-]+)/?", last["path"])
    if not match:
        return False, f"last shop page is {last['path']}, not a category listing"
    shown = sorted(p["id"] for p in listing_filter(exp["products"], match.group(1), last.get("query") or {}))
    return shown == exp["expected"], f"last listing {last['path']} {last.get('query')} shows {len(shown)}, want {len(exp['expected'])}"


def _fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": BENCH_UA})
    with urllib.request.urlopen(request, timeout=20) as r:
        return r.read().decode("utf-8", errors="replace")


def _strip_tags(text):
    return re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", " ", text))).strip()


def parse_arxiv_version(page, version):
    found = re.search(rf"\[v{version}\]\s*(?:</[^>]+>\s*)*([A-Z][a-z]{{2}}, \d{{1,2}} [A-Z][a-z]{{2}} \d{{4}})", page)
    if not found:
        found = re.search(rf"\[v{version}\]\s*([A-Z][a-z]{{2}}, \d{{1,2}} [A-Z][a-z]{{2}} \d{{4}})", _strip_tags(page))
    if not found:
        raise ValueError(f"no v{version} line on the abstract page")
    return datetime.datetime.strptime(found.group(1), "%a, %d %b %Y").date().isoformat()


def parse_arxiv_econ(page):
    found = re.findall(r"<h4>\s*(econ\.[A-Z]{2})\s*<span>\s*\(([^)]+)\)\s*</span>", page)
    if len(found) < 2:
        raise ValueError("no econ categories on the taxonomy page")
    return {code: name.strip() for code, name in found}


def parse_cambridge_meanings(page):
    defs = [_strip_tags(m) for m in re.findall(r'class="def ddef_d db">(.*?)</div>', page, re.S)]
    unique = list(dict.fromkeys(d.rstrip(": ") for d in defs if d))
    if not unique:
        raise ValueError("no definitions on the dictionary page")
    return len(unique)


def web_expectations(task_id):
    """Values the public tasks are judged against, read by the verifier from the site just before the run."""
    if task_id == "hp1":
        return {"hp1_v3": parse_arxiv_version(_fetch("https://arxiv.org/abs/2303.08774"), 3)}
    if task_id == "hp2":
        return {"hp2_econ": parse_arxiv_econ(_fetch("https://arxiv.org/category_taxonomy"))}
    if task_id == "hp3":
        return {"hp3_count": parse_cambridge_meanings(_fetch("https://dictionary.cambridge.org/dictionary/english/unblemished"))}
    return {}


def date_mentions(answer):
    out = set()
    text = re.sub(r"\s+", " ", answer.replace(",", " "))
    for fmt, rx in (("%d %b %Y", r"\b\d{1,2} [A-Z][a-z]{2,8} \d{4}\b"), ("%b %d %Y", r"\b[A-Z][a-z]{2,8} \d{1,2} \d{4}\b"),
                    ("%Y-%m-%d", r"\b\d{4}-\d{2}-\d{2}\b")):
        for m in re.findall(rx, text):
            for f in (fmt, fmt.replace("%b", "%B")):
                try:
                    out.add(datetime.datetime.strptime(re.sub(r"\s+", " ", m), f).date().isoformat())
                except ValueError:
                    pass
    return out


def verify_public(task_id, exp, answer):
    if task_id == "hp1":
        return exp["hp1_v3"] in date_mentions(answer), f"want v3 date {exp['hp1_v3']}"
    if task_id == "hp2":
        econ = exp["hp2_econ"]
        low = answer.lower()
        ok = all(code.lower() in low and name.lower() in low for code, name in econ.items())
        return ok, f"want {econ}"
    if task_id == "hp3":
        nums = [int(n) for n in re.findall(r"(?<![\d.])\d+(?![\d.])", answer)]
        words = {"one": 1, "two": 2, "three": 3, "four": 4, "five": 5}
        nums += [v for k, v in words.items() if re.search(rf"\b{k}\b", answer.lower())]
        return nums[:1] == [exp["hp3_count"]] or set(nums) == {exp["hp3_count"]}, f"want {exp['hp3_count']}"
    return False, "unknown public task"


LOCAL_VERIFIERS = {"hw1": verify_hw1, "hw2": verify_hw2, "hw3": verify_hw3, "hw4": verify_hw4, "hw5": verify_hw5,
                   "hw6": verify_hw6, "hw7": verify_hw7}


def verify_web(task, ctx, answer, state):
    """Signature of run.py's verify(task, ctx, answer, state)."""
    tid = task["id"]
    if tid.startswith("hp"):
        return verify_public(tid, ctx["exp"], answer)
    w = world(int(ctx["exp"]["seed"]))
    return LOCAL_VERIFIERS[tid](w, answer, state)


def web_prompt(tid):
    """Prompts with {base} and {exp[...]} placeholders, filled by run.py from the server's /__reset reply."""
    return {
        "hw1": ("In Google Chrome, open {base}/shop/ , find the product '{exp[hw1_product]}' and list every reviewer "
                "who says its ear cups are too small. Reply with their names, separated by commas."),
        "hw2": ("Today is {exp[hw2_today_long]}. In Google Chrome, open {base}/shop/ , go to my account's order history, "
                "and tell me how many orders with status Complete I placed from {exp[hw2_start_long]} through "
                "{exp[hw2_today_long]}, and the total amount of those orders. Reply as: COUNT | TOTAL."),
        "hw3": ("In Google Chrome, open the store admin at {base}/admin/ and reduce the price of every "
                "{exp[hw3_color]} {exp[hw3_family]} product, in every size, by $5. Leave every other product unchanged."),
        "hw4": "In Google Chrome, open the forum at {base}/forum/ and upvote the newest post in the books forum.",
        "hw5": ("In Google Chrome, open {base}/forum/f/pics/{exp[hw5_post]} . Reply to the comment written by the site "
                "manager (the forum moderator, marked MOD) with \"thanks! I am a big fan of your website.\""),
        "hw6": ("In Google Chrome, open the code hosting site at {base}/git/ . Create an issue in the a11yproject.com "
                "project with the title \"401 bad gateway\", assign it to {exp[hw6_assignee]}, and set its due date to "
                "the end of {exp[hw6_due_text]}."),
        "hw7": ("In Google Chrome, open {base}/shop/ and show me the products under $25 in the Women Shoes category. "
                "Leave the browser on that list."),
        "hp1": ("In Google Chrome, open https://arxiv.org/ , find the paper 'GPT-4 Technical Report' and tell me when "
                "its version 3 (v3) was submitted." + PUBLIC_RULES),
        "hp2": ("In Google Chrome, open https://arxiv.org/ and find which categories Economics includes on arXiv, with "
                "their abbreviations." + PUBLIC_RULES),
        "hp3": ("In Google Chrome, open https://dictionary.cambridge.org/ , look up the word \"unblemished\" and tell me "
                "how many meanings the dictionary gives for it. Reply with the number." + PUBLIC_RULES),
    }[tid]


def reset_payload(seed):
    """What the server's /__reset returns: the seed plus the short values the prompts need."""
    w = world(seed)
    today, start = (datetime.date.fromisoformat(w["hw2"][k]) for k in ("today", "start"))
    return {"seed": seed, "hw1_product": w["hw1"]["product"], "hw2_today_long": long_date(today),
            "hw2_start_long": long_date(start), "hw3_color": w["hw3"]["color"], "hw3_family": w["hw3"]["family"],
            "hw5_post": w["hw5"]["post"]["id"], "hw6_assignee": w["hw6"]["assignee"]["name"],
            "hw6_due_text": w["hw6"]["due_text"]}


WEB_TASKS = [{"id": tid, "channels": WEB_CHANNELS, "prompt": web_prompt(tid)}
             for tid in ["hw1", "hw2", "hw3", "hw4", "hw5", "hw6", "hw7", "hp1", "hp2", "hp3"]]


# ---------------------------------------------------------------- native

def _bench_tasks():
    """bench/tasks.py, reused when the driver already imported it as `tasks`, else loaded as `bench_tasks`.
    It is never imported as `tasks` here: the browser bench's run.py has its own `tasks` module."""
    loaded = sys.modules.get("tasks")
    if loaded is not None and Path(getattr(loaded, "__file__", "")).resolve() == BENCH / "tasks.py":
        return loaded
    if "bench_tasks" not in sys.modules:
        spec = importlib.util.spec_from_file_location("bench_tasks", BENCH / "tasks.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        sys.modules["bench_tasks"] = module
    return sys.modules["bench_tasks"]


_bt = _bench_tasks()
BENCH_DIR, FORBIDDEN, Task, macctl = _bt.BENCH_DIR, _bt.FORBIDDEN, _bt.Task, _bt.macctl

FORBIDDEN.update({"pmset", "plutil", "ioreg", "sips", "textutil", "readlink", "systemsetup", "sysadminctl", "swift",
                  "swiftc", "qlmanage", "automator", "screencapture", "osascript", "open", "defaults", "curl", "wget",
                  "node", "ruby", "perl"})
PROBE_SRC = HERE / "ho_probe.swift"
PROBE_BIN = HERE / "out" / "ho_probe"
NATIVE_CAP_S = 300
READ_ONLY = " Only read the values. Do not change any setting."


def build_probe():
    if not PROBE_BIN.exists() or PROBE_BIN.stat().st_mtime < PROBE_SRC.stat().st_mtime:
        PROBE_BIN.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["swiftc", "-O", str(PROBE_SRC), "-o", str(PROBE_BIN)], check=True, capture_output=True)


def probe(*args):
    build_probe()
    out = subprocess.run([str(PROBE_BIN), *args], capture_output=True, text=True, timeout=30)
    if out.returncode != 0:
        raise RuntimeError(f"ho_probe {args[0]} failed: {out.stderr.strip()[:200]}")
    return json.loads(out.stdout)


def bench_title(title, prefixes=("seed-ho-", "bench-ho-")):
    return str(title).startswith(prefixes)


def app_windows(app):
    try:
        return json.loads(macctl("windows", app) or "[]")
    except (json.JSONDecodeError, subprocess.TimeoutExpired):
        return None


def app_running(app):
    return subprocess.run(["pgrep", "-x", app], capture_output=True).returncode == 0


def close_bench_app(app):
    """Quit the app only when every window it shows is a bench window; never touch anything else."""
    if not app_running(app):
        return
    titles = app_windows(app)
    if titles is None or not all(bench_title(t) for t in titles):
        raise RuntimeError(f"{app} has a window that is not a bench window open; not touching it")
    macctl("close-discard", app, "seed-ho-", "bench-ho-", timeout=60)
    macctl("quit", app)
    for _ in range(50):
        if not app_running(app):
            return
        time.sleep(0.2)
    raise RuntimeError(f"{app} did not quit")


def open_bench_document(app, path):
    """Open one bench file in a freshly started app, with window restoration off, and check that
    the bench document is the only window it shows."""
    if BENCH_DIR not in Path(path).resolve().parents:
        raise ValueError(f"refusing to open a file outside {BENCH_DIR}: {path}")
    close_bench_app(app)
    subprocess.run(["open", "-a", app, str(path), "--args", "-ApplePersistenceIgnoreState", "YES"], check=True, timeout=15)
    for _ in range(75):
        focused = json.loads(macctl("focused", app) or "{}")
        if str(focused.get("window", "")).startswith(Path(path).stem):
            break
        time.sleep(0.2)
    else:
        raise RuntimeError(f"{app} did not open the bench document")
    titles = app_windows(app) or []
    if not titles or not all(bench_title(t) for t in titles):
        close_bench_app(app)
        raise RuntimeError(f"{app} opened a window that is not the bench document")


class BenchFileTask(Task):
    """A task on one file the bench creates and opens by its full path."""
    suffix = ".rtf"

    def setup(self):
        self.folder = BENCH_DIR / f"bench-ho-{self.key}-{self.run_id}"
        shutil.rmtree(self.folder, ignore_errors=True)
        self.folder.mkdir(parents=True)
        self.path = self.folder / f"seed-ho-{self.key}-{self.run_id}{self.suffix}"
        self.write_seed()
        self.mtime0 = self.path.stat().st_mtime
        open_bench_document(self.app, self.path)

    def cleanup(self):
        close_bench_app(self.app)
        shutil.rmtree(self.folder, ignore_errors=True)

    def saved(self):
        return self.path.exists() and self.path.stat().st_mtime > self.mtime0


def write_rtf(path, paragraphs):
    """paragraphs: [(html_inline_text, is_heading)] -> RTF via textutil, the way TextEdit writes it."""
    body = "".join(f"<h1>{t}</h1>" if head else f"<p>{t}</p>" for t, head in paragraphs)
    src = path.with_suffix(".html")
    src.write_text(f"<!doctype html><html><head><meta charset=utf-8></head><body>{body}</body></html>")
    subprocess.run(["textutil", "-convert", "rtf", str(src), "-output", str(path)], check=True, capture_output=True, timeout=30)
    src.unlink()


def paras(doc):
    return [p for p in doc["paragraphs"] if p["text"].strip()]


def doc_text(doc):
    return [p["text"] for p in paras(doc)]


def is_sub(run):
    return run.get("script", 0) < 0 or run.get("baseline", 0) < 0


H2O_DOC = [("Lab notes, week {n}", True), ("Pure water has the formula H2O and boils at 100 degrees.", False),
           ("Carbon dioxide, CO2, is a product of combustion.", False),
           ("Every H2O molecule holds two hydrogen atoms and one oxygen atom.", False)]
OUTLINE_DOC = [("Course outline: Introduction to Data Journalism", True),
               ("Week one covers finding stories in public records and spreadsheets.", False),
               ("Week two covers cleaning data and checking every number twice.", False),
               ("Week three covers charts that explain instead of decorate.", False),
               ("The final week repeats the material of weeks one to three as a general review.", False)]
HEADING_DOC = [("Quarterly Garden Club Report", True),
               ("Membership grew by twelve households this quarter.", False),
               ("The seed swap moved to the library on the first Saturday of each month.", False),
               ("Volunteers are still needed for the autumn bulb planting.", False)]
CAPS_DOC = [("notes from the harbour walk", False),
            ("we started at the old lighthouse and followed the sea wall north", False),
            ("the tide was low so the rock pools were full of crabs and small fish", False)]


def capitalize_words(text):
    return " ".join(w[:1].upper() + w[1:] for w in text.split(" "))


class SubscriptH2O(BenchFileTask):
    key = "hn1"
    app = "TextEdit"

    def write_seed(self):
        self.paras = [(t.replace("{n}", str(int(self.run_id[-2:], 16) % 40 + 1)), h) for t, h in H2O_DOC]
        write_rtf(self.path, self.paras)

    def prompt(self):
        return (f"TextEdit has the document `{self.path.name}` open. Change the 2 in every \"H2O\" in it to a subscript, "
                "leave everything else as it is, and save the document under its current name. Reply DONE when it is saved.")

    def verify(self, answer):
        if not self.saved():
            return False, "the document was not saved"
        doc = probe("rtf", str(self.path))
        return judge_subscript(doc, [t for t, _ in self.paras])


def judge_subscript(doc, want_text):
    if doc_text(doc) != want_text:
        return False, f"text changed: {doc_text(doc)}"
    for p in paras(doc):
        pos = 0
        h2o = {m.start() + 1 for m in re.finditer("H2O", p["text"])}
        for run in p["runs"]:
            for i, _ in enumerate(run["text"]):
                if is_sub(run) != ((pos + i) in h2o):
                    return False, f"wrong subscript at {p['text']!r} char {pos + i}"
            pos += len(run["text"])
    return True, "every H2O 2 is subscript, nothing else is"


class StrikeLastParagraph(BenchFileTask):
    key = "hn2"
    app = "TextEdit"

    def write_seed(self):
        write_rtf(self.path, OUTLINE_DOC)

    def prompt(self):
        return (f"TextEdit has the document `{self.path.name}` open. Its last paragraph is redundant: put a strike-through "
                "on the words of the last paragraph only, and save the document under its current name. Reply DONE when it is saved.")

    def verify(self, answer):
        if not self.saved():
            return False, "the document was not saved"
        return judge_strike(probe("rtf", str(self.path)), [t for t, _ in OUTLINE_DOC])


def judge_strike(doc, want_text):
    if doc_text(doc) != want_text:
        return False, f"text changed: {doc_text(doc)}"
    kept = paras(doc)
    for k, p in enumerate(kept):
        last = k == len(kept) - 1
        for run in p["runs"]:
            if run["text"].strip() and bool(run.get("strike")) != last:
                return False, f"strike-through wrong on {run['text'][:40]!r}"
    return True, "only the last paragraph is struck through"


class CenterHeading(BenchFileTask):
    key = "hn3"
    app = "TextEdit"

    def write_seed(self):
        write_rtf(self.path, HEADING_DOC)

    def prompt(self):
        return (f"TextEdit has the document `{self.path.name}` open. Center align its heading (the first line), leave the "
                "other paragraphs as they are, and save the document under its current name. Reply DONE when it is saved.")

    def verify(self, answer):
        if not self.saved():
            return False, "the document was not saved"
        return judge_center(probe("rtf", str(self.path)), [t for t, _ in HEADING_DOC])


def judge_center(doc, want_text):
    if doc_text(doc) != want_text:
        return False, f"text changed: {doc_text(doc)}"
    aligns = [p["alignment"] for p in paras(doc)]
    ok = aligns[0] == "center" and all(a != "center" for a in aligns[1:])
    return ok, f"alignments {aligns}"


class CapitalizeWords(BenchFileTask):
    key = "hn4"
    app = "TextEdit"

    def write_seed(self):
        write_rtf(self.path, CAPS_DOC)

    def prompt(self):
        return (f"TextEdit has the document `{self.path.name}` open. Make the first letter of each word in it uppercase, "
                "and save the document under its current name. Reply DONE when it is saved.")

    def verify(self, answer):
        if not self.saved():
            return False, "the document was not saved"
        return judge_caps(probe("rtf", str(self.path)), [t for t, _ in CAPS_DOC])


def judge_caps(doc, original):
    want = [capitalize_words(t) for t in original]
    got = [t.rstrip() for t in doc_text(doc)]
    return got == want, f"text {got}"


IMG_W, IMG_H = 600, 400


def seed_pixel(x, y):
    if 40 <= x < 140 and 40 <= y < 140:
        return (0, 200, 0)
    return (220, 30, 30) if x < IMG_W * 0.4 else (30, 30, 220)


def write_png(path, width, height, pixel):
    rows = b"".join(b"\x00" + bytes(c for x in range(width) for c in pixel(x, y)) for y in range(height))
    chunk = lambda kind, data: (len(data).to_bytes(4, "big") + kind + data
                                + zlib.crc32(kind + data).to_bytes(4, "big"))
    header = width.to_bytes(4, "big") + height.to_bytes(4, "big") + bytes([8, 2, 0, 0, 0])
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


def color_class(rgb):
    r, g, b = rgb[:3]
    top = max(r, g, b)
    if top < 120:
        return "dark"
    return {r: "red", g: "green", b: "blue"}[top] if sorted((r, g, b))[1] < top * 0.6 else "mixed"


GRID = (12, 8)


def expected_grid(transform):
    """Colour class at each grid cell centre of the seed image after `transform` maps output (u, v) in [0,1) to input."""
    cols, rows = GRID
    out = []
    for j in range(rows):
        row = []
        for i in range(cols):
            u, v = transform((i + 0.5) / cols, (j + 0.5) / rows)
            row.append(color_class(seed_pixel(int(u * IMG_W), int(v * IMG_H))))
        out.append(row)
    return out


def grid_classes(probe_out):
    return [[color_class(c) for c in row] for row in probe_out["grid"]]


def grid_match(got, want, tolerance=2):
    misses = sum(1 for g_row, w_row in zip(got, want) for g, w in zip(g_row, w_row) if g != w)
    return misses <= tolerance, misses


class ImageTask(BenchFileTask):
    app = "Preview"
    suffix = ".png"

    def write_seed(self):
        write_png(self.path, IMG_W, IMG_H, seed_pixel)

    def read(self):
        if self.path.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            return None
        return probe("pixels", str(self.path), str(GRID[0]), str(GRID[1]))


class FlipHorizontal(ImageTask):
    key = "hn5"

    def prompt(self):
        return (f"Preview has the image `{self.path.name}` open. Mirror it horizontally (flip it left to right) and save "
                "it under its current name and format. Reply DONE when it is saved.")

    def verify(self, answer):
        if not self.saved():
            return False, "the image was not saved"
        return judge_flip(self.read())


def judge_flip(img):
    if img is None:
        return False, "the file is no longer a PNG"
    if (img["width"], img["height"]) != (IMG_W, IMG_H):
        return False, f"size changed to {img['width']}x{img['height']}"
    ok, misses = grid_match(grid_classes(img), expected_grid(lambda u, v: (1 - u, v)))
    return ok, f"{misses} grid cells differ from the mirrored seed"


class ResizeHeight(ImageTask):
    key = "hn6"

    def prompt(self):
        return (f"Preview has the image `{self.path.name}` open. Resize it to a height of 512 pixels while keeping its "
                "aspect ratio, and save it under its current name and format. Reply DONE when it is saved.")

    def verify(self, answer):
        if not self.saved():
            return False, "the image was not saved"
        return judge_resize(self.read())


def judge_resize(img):
    if img is None:
        return False, "the file is no longer a PNG"
    want_w = round(IMG_W * 512 / IMG_H)
    if img["height"] != 512 or abs(img["width"] - want_w) > 1:
        return False, f"size {img['width']}x{img['height']}, want {want_w}x512"
    ok, misses = grid_match(grid_classes(img), expected_grid(lambda u, v: (u, v)))
    return ok, f"{misses} grid cells differ from the scaled seed"


class ReadOnlySettingsTask(Task):
    app = "System Settings"

    def setup(self):
        self.before = self.state()

    def verify(self, answer):
        after = self.state()
        if after != self.before:
            return False, f"SETTING CHANGED DURING THE RUN: {self.before} -> {after}"
        return self.judge(answer, self.before)


def current_timezone():
    link = subprocess.run(["readlink", "/etc/localtime"], capture_output=True, text=True).stdout.strip()
    return link.split("zoneinfo/")[-1] if "zoneinfo/" in link else link


class TimeZoneRead(ReadOnlySettingsTask):
    key = "hn7"

    def prompt(self):
        return ("Open System Settings, go to General > Date & Time, and report the time zone shown there. "
                "Reply with the time zone only." + READ_ONLY)

    def state(self):
        return current_timezone()

    def judge(self, answer, zone):
        return judge_timezone(answer, zone, probe("tz", zone))


def judge_timezone(answer, zone, names):
    low = _norm(answer).lower()
    city = zone.split("/")[-1].replace("_", " ")
    accepted = [zone, city] + [n for n in names.get("names", []) if n]
    ok = any(a.lower() in low for a in accepted)
    return ok, f"want one of {accepted}"


def display_sleep_ac():
    out = subprocess.run(["pmset", "-g", "custom"], capture_output=True, text=True).stdout
    section, value = None, None
    for line in out.splitlines():
        if line.endswith("Power:"):
            section = "ac" if line.startswith("AC") else "battery"
        elif line.strip().startswith("displaysleep") and section == "ac":
            value = int(line.split()[-1])
    return value


def minutes_said(answer):
    low = answer.lower()
    if re.search(r"\bnever\b", low):
        return 0
    found = re.search(r"(\d+)\s*(hour|hr|minute|min)", low)
    if not found:
        return None
    return int(found.group(1)) * (60 if found.group(2).startswith("h") else 1)


class DisplayOffRead(ReadOnlySettingsTask):
    key = "hn8"

    def prompt(self):
        return ("Open System Settings, go to Lock Screen, and report after how long the display turns off when the Mac "
                "is inactive while on power adapter. Reply with the value shown (for example: For 10 minutes, or Never)."
                + READ_ONLY)

    def state(self):
        return display_sleep_ac()

    def judge(self, answer, minutes):
        if minutes is None:
            return False, "pmset reports no AC displaysleep value"
        return minutes_said(answer) == minutes, f"want {'never' if minutes == 0 else f'{minutes} minutes'}"


NATIVE_TASKS = {t.key: t for t in [SubscriptH2O, StrikeLastParagraph, CenterHeading, CapitalizeWords, FlipHorizontal,
                                   ResizeHeight, TimeZoneRead, DisplayOffRead]}
NATIVE_APPS = ["TextEdit", "Preview", "System Settings"]
