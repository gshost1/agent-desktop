#!/usr/bin/env python3
"""Local test site for the browser/computer-use benchmark.

Serves seeded task pages on 127.0.0.1 only. Every value an agent must read or
produce is derived from the seed set by GET /__reset?seed=N, and every
submission is recorded so the harness can judge success by independent
observation instead of by the agent's claim.
"""
import html
import json
import random
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

QUOTES = [
    "The most damaging phrase in the language is: it has always been done this way.",
    "Simplicity is prerequisite for reliability.",
    "Programs must be written for people to read, and only incidentally for machines to execute.",
    "The purpose of abstraction is not to be vague, but to create a new semantic level.",
    "Premature optimization is the root of all evil in programming.",
    "Testing shows the presence, not the absence of bugs.",
]
COUNTRIES = ["Argentina", "Brazil", "Canada", "Denmark", "Estonia", "Finland",
             "Germany", "Japan", "Kenya", "Norway", "Portugal", "Spain"]

STATE = {"seed": 0, "events": []}
LOCK = threading.Lock()


def expected(seed):
    rng = random.Random(seed)
    items = [(f"Item {chr(65 + i)}", rng.randint(2, 9), rng.randint(300, 9900) / 100) for i in range(6)]
    subtotal = round(sum(q * p for _, q, p in items), 2)
    tax = round(subtotal * 0.08, 2)
    total = round(subtotal + tax + 7.5, 2)
    products = []
    for i in range(300):
        products.append({"sku": f"SKU-{rng.randint(10000, 99999)}",
                         "name": f"{rng.choice(['Red','Blue','Green','Black','Silver'])} "
                                 f"{rng.choice(['Widget','Gadget','Sprocket','Bracket','Valve'])} "
                                 f"{rng.choice(['Small','Medium','Large','XL'])} {i:03d}",
                         "stock": rng.randint(0, 999)})
    target = products[rng.randint(150, 290)]
    return {
        "items": items, "subtotal": subtotal, "tax": tax, "total": f"{total:.2f}",
        "code": f"CNF-{rng.randint(100000, 999999)}",
        "products": products, "w4_name": target["name"], "w4_stock": str(target["stock"]),
        "quote": QUOTES[rng.randrange(len(QUOTES))],
        "x2_a": rng.randint(23, 97), "x2_b": rng.randint(23, 97),
        "report": f"report-{rng.randint(1000, 9999)}.csv",
    }


def page(title, body):
    return (f"<!doctype html><html lang=en><head><meta charset=utf-8><title>{html.escape(title)}</title>"
            "<style>body{font:15px system-ui;margin:2em;max-width:900px}td,th{padding:4px 10px;border-bottom:1px solid #ddd}"
            "label{display:block;margin:.6em 0}</style></head>"
            f"<body><nav><a href=/>Home</a> · <a href=/help>Help</a> · <a href=/about>About</a></nav><h1>{html.escape(title)}</h1>{body}</body></html>")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, status, body, ctype="text/html; charset=utf-8", extra=None):
        data = body.encode() if isinstance(body, str) else body
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)

    def record(self, kind, fields):
        with LOCK:
            STATE["events"].append({"kind": kind, "fields": fields, "ua": self.headers.get("User-Agent", "")})

    def do_GET(self):
        url = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(url.query).items()}
        e = expected(STATE["seed"])
        p = url.path
        if p == "/__reset":
            with LOCK:
                STATE["seed"] = int(q.get("seed", "0"))
                STATE["events"] = []
            exp = expected(STATE["seed"])
            exp.pop("products")
            exp.pop("items")
            return self.send(200, json.dumps(exp), "application/json")
        if p == "/__state":
            with LOCK:
                return self.send(200, json.dumps(STATE), "application/json")
        if p == "/":
            links = "".join(f"<li><a href=/{t}/>{t}</a></li>" for t in ["w1", "w2", "w3", "w4", "x1", "x2", "x3"])
            return self.send(200, page("Bench home", f"<ul>{links}</ul>"))
        if p in ("/help", "/about"):
            return self.send(200, page(p[1:].title(), "<p>Nothing to see here.</p>"))
        if p == "/w1/":
            rows = "".join(f"<tr><td>{n}</td><td>{qn}</td><td>${pr:.2f}</td><td>${qn*pr:.2f}</td></tr>" for n, qn, pr in e["items"])
            body = (f"<p>Order placed by a returning customer. Reference A-{STATE['seed']:04d}.</p>"
                    f"<table><tr><th>Item</th><th>Qty</th><th>Unit</th><th>Line</th></tr>{rows}</table>"
                    f"<p>Subtotal: ${e['subtotal']:.2f}</p><p>Tax (8%): ${e['tax']:.2f}</p><p>Shipping: $7.50</p>"
                    f"<p><strong>Order total: <span id=total>${e['total']}</span></strong></p>")
            return self.send(200, page("Order summary", body))
        if p == "/w2/":
            opts = "".join(f"<option>{c}</option>" for c in COUNTRIES)
            body = ("<form method=post action=/w2/submit>"
                    "<label>Full name <input name=name autocomplete=off></label>"
                    "<label>Email <input name=email type=email autocomplete=off></label>"
                    f"<label>Country <select name=country><option value=''>Choose…</option>{opts}</select></label>"
                    "<fieldset><legend>Shipping</legend><label><input type=radio name=shipping value=standard checked> Standard</label>"
                    "<label><input type=radio name=shipping value=express> Express</label></fieldset>"
                    "<label><input type=checkbox name=gift value=yes> Gift wrap</label>"
                    "<label>Notes <textarea name=notes rows=3></textarea></label>"
                    "<button type=submit>Place order</button></form>")
            return self.send(200, page("Shipping details", body))
        if p == "/w3/":
            body = ("<p>Step 1 of 3: choose a plan.</p><form action=/w3/2>"
                    "<label><input type=radio name=plan value=Basic checked> Basic</label>"
                    "<label><input type=radio name=plan value=Pro> Pro</label>"
                    "<label><input type=radio name=plan value=Team> Team</label><button>Next</button></form>")
            return self.send(200, page("Subscribe", body))
        if p == "/w3/2":
            plan = html.escape(q.get("plan", ""))
            body = (f"<p>Step 2 of 3: plan {plan}. How many seats?</p><form action=/w3/3>"
                    f"<input type=hidden name=plan value='{plan}'>"
                    "<label>Seats <input name=seats type=number min=1 max=50 value=1></label>"
                    "<label>Coupon <input name=coupon></label><button>Next</button></form>")
            return self.send(200, page("Subscribe", body))
        if p == "/w3/3":
            plan, seats = html.escape(q.get("plan", "")), html.escape(q.get("seats", ""))
            body = (f"<p>Step 3 of 3: review. Plan {plan}, {seats} seats.</p>"
                    f"<form method=post action=/w3/done><input type=hidden name=plan value='{plan}'>"
                    f"<input type=hidden name=seats value='{seats}'>"
                    "<label><input type=checkbox name=terms value=yes> I accept the terms</label><button>Confirm</button></form>")
            return self.send(200, page("Subscribe", body))
        if p == "/w4/":
            rows = "".join(f"<tr><td>{x['sku']}</td><td>{x['name']}</td><td>{x['stock']}</td></tr>" for x in e["products"])
            body = ("<p>Warehouse inventory. Use your browser's find to search.</p>"
                    f"<table><tr><th>SKU</th><th>Product</th><th>In stock</th></tr>{rows}</table>")
            return self.send(200, page("Inventory", body))
        if p == "/x1/":
            body = ("<article><p>From our engineering handbook, chapter 3.</p>"
                    f"<blockquote id=quote>{html.escape(e['quote'])}</blockquote>"
                    "<p>Discuss with your team.</p></article>")
            return self.send(200, page("Quote of the day", body))
        if p == "/x2/":
            body = (f"<p>Units ordered: <b id=a>{e['x2_a']}</b></p><p>Unit price (USD): <b id=b>{e['x2_b']}</b></p>"
                    "<p>Compute the extended price yourself.</p>")
            return self.send(200, page("Pricing", body))
        if p == "/x3/":
            body = f"<p>The weekly report is ready.</p><p><a id=dl href=/x3/{e['report']}>Download {e['report']}</a></p>"
            return self.send(200, page("Reports", body))
        if p.startswith("/x3/report-"):
            self.record("download", {"file": p.rsplit("/", 1)[-1]})
            csv = "week,orders,revenue\n44,120,9832.10\n45,131,10211.55\n"
            return self.send(200, csv, "text/csv", {"Content-Disposition": f"attachment; filename={e['report']}"})
        return self.send(404, page("Not found", "<p>No such page.</p>"))

    def do_POST(self):
        url = urlparse(self.path)
        n = int(self.headers.get("Content-Length", "0") or 0)
        fields = {k: v[0] for k, v in parse_qs(self.rfile.read(n).decode()).items()}
        if url.path == "/w2/submit":
            self.record("w2", fields)
            return self.send(200, page("Thank you", "<p>Your order was placed.</p>"))
        if url.path == "/w3/done":
            self.record("w3", fields)
            if fields.get("terms") != "yes":
                return self.send(200, page("Subscribe", "<p>You must accept the terms. <a href=/w3/>Start over</a></p>"))
            code = expected(STATE["seed"])["code"]
            return self.send(200, page("Subscribed", f"<p>Confirmation code: <strong id=code>{code}</strong></p>"))
        return self.send(404, page("Not found", ""))


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
