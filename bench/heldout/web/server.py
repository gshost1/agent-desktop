#!/usr/bin/env python3
"""Local site for the held-out web tasks: a shop, a store admin, a forum and a code host.

127.0.0.1 only. Every value comes from tasks_heldout.world(seed), set by GET /__reset?seed=N.
Every write (price save, vote, comment, issue) and every page view is recorded, and
GET /__state returns them, so verifiers judge by what the site saw, not by the agent's claim.

Usage: python3 -I heldout/web/server.py [port]   (default 8792)
"""
import datetime
import html
import json
import math
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import tasks_heldout as th  # noqa: E402

LOCK = threading.Lock()
STATE = {}
CACHE = {}


def reset(seed):
    with LOCK:
        STATE.clear()
        STATE.update({"seed": seed, "events": [], "navs": [], "admin_prices": {}, "votes": {}, "comments": [],
                      "issues": []})
        CACHE.clear()
        CACHE["w"] = th.world(seed)
        STATE["admin_prices"] = {str(p["id"]): p["price"] for p in CACHE["w"]["hw3"]["products"]}


reset(0)
e = html.escape


def page(title, body, section=""):
    nav = " · ".join(f"<a href=/{s}/>{n}</a>" for s, n in [("shop", "Shop"), ("admin", "Admin"), ("forum", "Forum"), ("git", "Git")])
    return (f"<!doctype html><html lang=en><head><meta charset=utf-8><title>{e(title)}</title><style>"
            "body{font:15px system-ui;margin:1.5em;max-width:1000px}td,th{padding:4px 8px;border-bottom:1px solid #ddd;"
            "text-align:left}label{display:block;margin:.5em 0}.mod{background:#2a6;color:#fff;padding:0 4px;border-radius:3px;"
            "font-size:11px}.c{margin-left:1.5em;border-left:2px solid #ddd;padding-left:.8em}</style></head>"
            f"<body><nav>{nav}</nav><h1>{e(title)}</h1>{body}</body></html>")


def pager(base, query, page_no, pages, key="p"):
    links = []
    for n in range(1, pages + 1):
        q = dict(query, **{key: str(n)})
        links.append(f"<b>{n}</b>" if n == page_no else f"<a href='{base}?{e(urlencode(q))}'>{n}</a>")
    return f"<p>Page: {' '.join(links)}</p>" if pages > 1 else ""


def chunk(items, page_no, size):
    pages = max(1, math.ceil(len(items) / size))
    page_no = min(max(1, page_no), pages)
    return items[(page_no - 1) * size: page_no * size], page_no, pages


def as_int(value, default=1):
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def hot_rank(post, now):
    age_h = (now - datetime.datetime.fromisoformat(post["created"])).total_seconds() / 3600
    return (post["score"] + STATE["votes"].get(str(post["id"]), 0)) / (age_h + 2) ** 1.5


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

    def redirect(self, to):
        self.send(303, "", extra={"Location": to})

    def record(self, kind, fields):
        with LOCK:
            STATE["events"].append({"kind": kind, "fields": fields, "t": datetime.datetime.now().isoformat()})

    def do_GET(self):
        url = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(url.query).items()}
        p = url.path
        if p == "/__reset":
            reset(as_int(q.get("seed"), 0))
            return self.send(200, json.dumps(th.reset_payload(STATE["seed"])), "application/json")
        if p == "/__state":
            with LOCK:
                return self.send(200, json.dumps(STATE), "application/json")
        if p == "/favicon.ico":
            return self.send(404, "")
        with LOCK:
            STATE["navs"].append({"path": p, "query": q})
        w = CACHE["w"]
        for prefix, fn in (("/shop", self.shop), ("/admin", self.admin), ("/forum", self.forum), ("/git", self.git)):
            if p == prefix or p.startswith(prefix + "/"):
                return fn(p[len(prefix):] or "/", q, w)
        if p == "/":
            return self.send(200, page("Bench sites", "<ul><li><a href=/shop/>Shop</a><li><a href=/admin/>Admin</a>"
                                                      "<li><a href=/forum/>Forum</a><li><a href=/git/>Git</a></ul>"))
        return self.send(404, page("Not found", "<p>No such page.</p>"))

    def all_products(self, w):
        headphones = {"id": 10001, "name": w["hw1"]["product"], "category": "headphones", "price": 89.0}
        return w["hw7"]["products"] + [headphones]

    def shop(self, p, q, w):
        products = self.all_products(w)
        cats = "".join(f"<li><a href=/shop/category/{s}/>{e(n)}</a></li>" for s, n in th.CATEGORY_NAMES.items())
        search = "<form action=/shop/search><input name=q placeholder='Search entire store'><button>Search</button></form>"
        if p == "/":
            return self.send(200, page("One Stop Market", f"{search}<h2>Categories</h2><ul>{cats}</ul>"
                                                          "<p><a href=/shop/account/>My Account</a></p>"))
        if p == "/search":
            term = q.get("q", "").lower().strip()
            hits = [x for x in products if term and all(t in x["name"].lower() for t in term.split())]
            rows = "".join(f"<li><a href=/shop/product/{x['id']}>{e(x['name'])}</a> {th.fmt_money(x['price'])}</li>" for x in hits)
            return self.send(200, page(f"Search results for '{q.get('q', '')}'", f"{search}<ul>{rows or '<li>No results.</li>'}</ul>"))
        if p.startswith("/category/"):
            slug = p.split("/")[2]
            if slug not in th.CATEGORY_NAMES:
                return self.send(404, page("Not found", ""))
            shown = th.listing_filter(products, slug, q)
            items, page_no, pages = chunk(shown, as_int(q.get("p")), 12)
            buckets = " · ".join(f"<a href='/shop/category/{slug}/?price_min={lo}&price_max={lo + 19.99:.2f}'>"
                                 f"${lo:.2f} - ${lo + 19.99:.2f}</a>" for lo in range(0, 100, 20))
            form = (f"<form action=/shop/category/{slug}/>Price from <input name=price_min size=5 value='{e(q.get('price_min', ''))}'> "
                    f"to <input name=price_max size=5 value='{e(q.get('price_max', ''))}'> "
                    "<select name=sort><option value=''>Position</option><option value=price>Price</option></select> "
                    "<button>Apply</button></form>")
            rows = "".join(f"<tr><td><a href=/shop/product/{x['id']}>{e(x['name'])}</a></td><td>{th.fmt_money(x['price'])}</td></tr>"
                           for x in items)
            body = (f"<p>Shop by price: {buckets}</p>{form}<p>{len(shown)} items</p>"
                    f"<table><tr><th>Product</th><th>Price</th></tr>{rows}</table>"
                    f"{pager(f'/shop/category/{slug}/', q, page_no, pages)}")
            return self.send(200, page(th.CATEGORY_NAMES[slug], body))
        if p.startswith("/product/"):
            pid = as_int(p.split("/")[2], 0)
            item = next((x for x in products if x["id"] == pid), None)
            if not item:
                return self.send(404, page("Not found", ""))
            body = f"<p>Price: {th.fmt_money(item['price'])}</p><button disabled>Add to Cart</button>"
            if pid == 10001:
                reviews, page_no, pages = chunk(w["hw1"]["reviews"], as_int(q.get("rp")), 8)
                rows = "".join(f"<div><p><b>{'★' * r['rating']}</b> Review by <b>{e(r['author'])}</b></p><p>{e(r['body'])}</p></div><hr>"
                               for r in reviews)
                body += (f"<h2>Customer Reviews ({len(w['hw1']['reviews'])})</h2>{rows}"
                         f"{pager(f'/shop/product/{pid}', q, page_no, pages, 'rp')}")
            return self.send(200, page(item["name"], body))
        if p in ("/account", "/account/"):
            return self.send(200, page("My Account", "<ul><li><a href=/shop/account/orders>My Orders</a><li><a href=/shop/account/wishlist>"
                                                      "My Wish List</a><li><a href=/shop/account/address>Address Book</a></ul>"))
        if p == "/account/orders":
            orders, page_no, pages = chunk(w["hw2"]["orders"], as_int(q.get("p")), 10)
            rows = "".join(f"<tr><td>{o['number']}</td><td>{datetime.date.fromisoformat(o['date']).strftime('%-m/%-d/%y')}</td>"
                           f"<td>{e(o['ship_to'])}</td><td>{th.fmt_money(o['total'])}</td><td>{o['status']}</td></tr>" for o in orders)
            body = ("<table><tr><th>Order #</th><th>Date</th><th>Ship To</th><th>Order Total</th><th>Status</th></tr>"
                    f"{rows}</table>{pager('/shop/account/orders', q, page_no, pages)}")
            return self.send(200, page("My Orders", body))
        if p in ("/account/wishlist", "/account/address"):
            return self.send(200, page(p.split("/")[-1].title(), "<p>Nothing here yet.</p>"))
        return self.send(404, page("Not found", ""))

    def admin(self, p, q, w):
        prices = STATE["admin_prices"]
        products = w["hw3"]["products"]
        if p == "/":
            return self.send(200, page("Dashboard", "<ul><li><a href=/admin/catalog/products>Catalog &gt; Products</a>"
                                                    "<li><a href=/admin/sales/orders>Sales &gt; Orders</a></ul>"))
        if p == "/sales/orders":
            return self.send(200, page("Orders", "<p>No orders to show.</p>"))
        if p == "/catalog/products":
            term = q.get("search", "").lower().strip()
            hits = [x for x in products if all(t in x["name"].lower() for t in term.split())]
            items, page_no, pages = chunk(hits, as_int(q.get("p")), 20)
            rows = "".join(f"<tr><td>{x['id']}</td><td>{e(x['name'])}</td><td>{e(x['sku'])}</td>"
                           f"<td>{th.fmt_money(prices[str(x['id'])])}</td><td>{x['qty']}</td><td>Enabled</td>"
                           f"<td><a href=/admin/catalog/product/{x['id']}/edit>Edit</a></td></tr>" for x in items)
            msg = "<p><b>You saved the product.</b></p>" if q.get("saved") else ""
            body = (f"{msg}<form action=/admin/catalog/products><input name=search value='{e(q.get('search', ''))}' "
                    "placeholder='Search by keyword'><button>Search</button></form>"
                    f"<p>{len(hits)} records found</p><table><tr><th>ID</th><th>Name</th><th>SKU</th><th>Price</th>"
                    f"<th>Quantity</th><th>Status</th><th>Action</th></tr>{rows}</table>"
                    f"{pager('/admin/catalog/products', q, page_no, pages)}")
            return self.send(200, page("Products", body))
        if p.startswith("/catalog/product/") and p.endswith("/edit"):
            pid = as_int(p.split("/")[3], 0)
            item = next((x for x in products if x["id"] == pid), None)
            if not item:
                return self.send(404, page("Not found", ""))
            body = (f"<form method=post action=/admin/catalog/product/{pid}/save>"
                    f"<label>Product Name <input name=name value='{e(item['name'])}' size=40></label>"
                    f"<label>SKU <input name=sku value='{e(item['sku'])}'></label>"
                    f"<label>Price ($) <input name=price value='{prices[str(pid)]:.2f}'></label>"
                    f"<label>Quantity <input name=qty value='{item['qty']}'></label>"
                    "<label>Enable Product <select name=status><option>Yes</option><option>No</option></select></label>"
                    "<button>Save</button> <a href=/admin/catalog/products>Back</a></form>")
            return self.send(200, page(item["name"], body))
        return self.send(404, page("Not found", ""))

    def forum(self, p, q, w):
        posts = w["hw4"]["posts"]
        now = datetime.datetime.fromisoformat(w["hw4"]["now"])
        parts = [x for x in p.split("/") if x]
        if not parts:
            forums = sorted({x["forum"] for x in posts})
            return self.send(200, page("Forums", "<ul>" + "".join(f"<li><a href=/forum/f/{f}>{f}</a>" for f in forums) + "</ul>"))
        if parts[0] == "f" and len(parts) == 2:
            name, sort = parts[1], q.get("sort", "hot")
            mine = [x for x in posts if x["forum"] == name]
            key = {"new": lambda x: x["created"], "top": lambda x: x["score"] + STATE["votes"].get(str(x["id"]), 0)}.get(
                sort, lambda x: hot_rank(x, now))
            mine.sort(key=key, reverse=True)
            tabs = " | ".join(f"<b>{s.title()}</b>" if s == sort else f"<a href='/forum/f/{name}?sort={s}'>{s.title()}</a>"
                              for s in ("hot", "new", "top"))
            rows = []
            for x in mine:
                score = x["score"] + STATE["votes"].get(str(x["id"]), 0)
                created = datetime.datetime.fromisoformat(x["created"])
                ago = int((now - created).total_seconds() // 3600)
                rows.append(f"<tr><td><form method=post action=/forum/vote style=display:inline><input type=hidden name=post value={x['id']}>"
                            f"<input type=hidden name=back value='/forum/f/{name}?sort={sort}'>"
                            f"<button name=dir value=up title=Upvote>▲</button> {score} "
                            f"<button name=dir value=down title=Downvote>▼</button></form></td>"
                            f"<td><a href=/forum/f/{name}/{x['id']}>{e(x['title'])}</a><br><small>Submitted {ago} hours ago by "
                            f"{e(x['author'])} · {x['comments']} comments</small></td></tr>")
            return self.send(200, page(f"/f/{name}", f"<p>Sort by: {tabs}</p><table>{''.join(rows)}</table>"))
        if parts[0] == "f" and len(parts) == 3:
            pid = as_int(parts[2], 0)
            t = w["hw5"]
            if pid == t["post"]["id"]:
                return self.send(200, page(t["post"]["title"], self.thread(t, pid)))
            post = next((x for x in posts if x["id"] == pid), None)
            if post:
                return self.send(200, page(post["title"], f"<p>Submitted by {e(post['author'])}</p><p>No comments yet.</p>"))
        return self.send(404, page("Not found", ""))

    def thread(self, t, pid):
        everything = t["comments"] + [dict(c, author="You") for c in STATE["comments"] if c.get("post") == pid]

        def render(parent):
            out = []
            for c in [c for c in everything if (c.get("parent") or None) == parent]:
                badge = " <span class=mod>MOD</span>" if c.get("mod") else ""
                form = (f"<details><summary>Reply</summary><form method=post action=/forum/comment>"
                        f"<input type=hidden name=post value={pid}><input type=hidden name=parent value={c['id']}>"
                        f"<textarea name=body rows=3 cols=60></textarea><br><button>Post</button></form></details>")
                out.append(f"<div class=c><p><b>{e(c['author'])}</b>{badge}</p><p>{e(c['body'])}</p>{form}{render(c['id'])}</div>")
            return "".join(out)

        top = (f"<form method=post action=/forum/comment><input type=hidden name=post value={pid}>"
               "<textarea name=body rows=3 cols=60 placeholder='Write a comment'></textarea><br><button>Post</button></form>")
        return f"<p>Submitted by {e(t['post']['author'])} in /f/{t['forum']}</p>{top}<h2>Comments</h2>{render(None)}"

    def git(self, p, q, w):
        g = w["hw6"]
        parts = [x for x in p.split("/") if x]
        if not parts:
            rows = "".join(f"<li><a href=/git/{a}/{b}>{a} / {b}</a></li>" for a, b in th.PROJECTS)
            return self.send(200, page("Projects", f"<ul>{rows}</ul>"))
        if len(parts) < 2 or (parts[0], parts[1]) not in th.PROJECTS:
            return self.send(404, page("Not found", ""))
        project = f"{parts[0]}/{parts[1]}"
        base = f"/git/{project}"
        rest = parts[2:]
        if not rest:
            return self.send(200, page(project, f"<ul><li><a href={base}/-/issues>Issues</a><li>Merge requests<li>Wiki</ul>"))
        if rest == ["-", "issues"]:
            mine = [i for i in STATE["issues"] if i["project"] == project]
            rows = "".join(f"<li>#{n + 1} {e(i['title'])}</li>" for n, i in enumerate(mine))
            return self.send(200, page(f"{project} issues", f"<p><a href={base}/-/issues/new>New issue</a></p><ul>{rows or '<li>No open issues.</li>'}</ul>"))
        if rest == ["-", "issues", "new"]:
            members = "".join(f"<option value={m['username']}>{e(m['name'])} (@{m['username']})</option>"
                              for m in sorted(g["members"], key=lambda m: m["name"]))
            labels = "".join(f"<label style=display:inline><input type=checkbox name=labels value='{e(x)}'> {e(x)}</label> "
                             for x in g["labels"])
            body = (f"<form method=post action={base}/-/issues><label>Title <input name=title size=60></label>"
                    "<label>Description <textarea name=description rows=5 cols=60></textarea></label>"
                    f"<label>Assignee <select name=assignee><option value=''>Unassigned</option>{members}</select></label>"
                    f"<fieldset><legend>Labels</legend>{labels}</fieldset>"
                    "<label>Due date <input type=date name=due></label><button>Create issue</button></form>")
            return self.send(200, page("New Issue", body))
        return self.send(404, page("Not found", ""))

    def do_POST(self):
        url = urlparse(self.path)
        n = int(self.headers.get("Content-Length", "0") or 0)
        raw = parse_qs(self.rfile.read(n).decode())
        f = {k: v[0] if len(v) == 1 else v for k, v in raw.items()}
        p = url.path
        self.record(p, f)
        if p.startswith("/admin/catalog/product/") and p.endswith("/save"):
            pid = p.split("/")[4]
            try:
                price = round(float(str(f.get("price", "")).replace("$", "").replace(",", "")), 2)
            except ValueError:
                return self.send(200, page("Error", "<p>Please enter a valid price.</p>"))
            with LOCK:
                if pid in STATE["admin_prices"]:
                    STATE["admin_prices"][pid] = price
            return self.redirect("/admin/catalog/products?saved=1")
        if p == "/forum/vote":
            post, step = str(f.get("post", "")), {"up": 1, "down": -1}.get(f.get("dir"), 0)
            with LOCK:
                current = STATE["votes"].get(post, 0)
                STATE["votes"][post] = 0 if current == step else step
            return self.redirect(f.get("back") or "/forum/")
        if p == "/forum/comment":
            body = str(f.get("body", "")).strip()
            if not body:
                return self.send(200, page("Error", "<p>Comment cannot be empty.</p>"))
            with LOCK:
                cid = 50000 + len(STATE["comments"])
                STATE["comments"].append({"id": cid, "post": as_int(f.get("post"), 0), "parent": as_int(f.get("parent"), 0) or None,
                                          "body": body})
            pid = f.get("post", "")
            return self.redirect(f"/forum/f/{CACHE['w']['hw5']['forum']}/{pid}")
        if p.startswith("/git/") and p.endswith("/-/issues"):
            project = "/".join(p.split("/")[2:4])
            labels = f.get("labels", [])
            with LOCK:
                STATE["issues"].append({"project": project, "title": str(f.get("title", "")), "assignee": f.get("assignee", ""),
                                        "due": f.get("due", ""), "labels": labels if isinstance(labels, list) else [labels],
                                        "description": f.get("description", "")})
            return self.redirect(f"/git/{project}/-/issues")
        return self.send(404, page("Not found", ""))


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8792
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
