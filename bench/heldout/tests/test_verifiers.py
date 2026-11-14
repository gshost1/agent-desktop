"""Unit tests of the held-out verifiers against fixture data. No GUI, no network, no socket.

Run: python3.11 -I -m unittest discover -s heldout/tests -p 'test_*.py'
"""
import datetime
import io
import json
import sys
import unittest
from pathlib import Path

HELDOUT = Path(__file__).resolve().parent.parent
FIX = HELDOUT / "tests" / "fixtures"
sys.path.insert(0, str(HELDOUT))
sys.path.insert(0, str(HELDOUT / "arms"))
sys.path.insert(0, str(HELDOUT / "web"))
import common  # noqa: E402
import server  # noqa: E402
import tasks_heldout as th  # noqa: E402


def fixture(name):
    return json.loads((FIX / name).read_text())


class FakeRequest(server.Handler):
    """Drives the server's handler without a socket: send() records instead of writing."""

    def __init__(self, path, body=b""):
        self.path = path
        self.headers = {"Content-Length": str(len(body))}
        self.rfile = io.BytesIO(body)
        self.sent = []

    def send(self, status, body, ctype="text/html; charset=utf-8", extra=None):
        self.sent.append((status, body if isinstance(body, str) else body.decode(), extra or {}))


def get(path):
    req = FakeRequest(path)
    req.do_GET()
    return req.sent[-1]


def post(path, form):
    from urllib.parse import urlencode
    req = FakeRequest(path, urlencode(form, doseq=True).encode())
    req.do_POST()
    return req.sent[-1]


def state():
    return json.loads(get("/__state")[1])


class WorldTests(unittest.TestCase):
    def test_world_is_deterministic_and_seed_dependent(self):
        self.assertEqual(json.dumps(th.world(7101)), json.dumps(th.world(7101)))
        self.assertNotEqual(th.world(7101)["hw2"]["today"] + th.world(7101)["hw1"]["product"],
                            th.world(7202)["hw2"]["today"] + th.world(7202)["hw1"]["product"] + "x")

    def test_every_arm_gets_the_same_seed_for_a_task_and_run(self):
        self.assertEqual(th.web_seed("hw1", 3), th.web_seed("hw1", 3))
        self.assertNotEqual(th.web_seed("hw1", 0), th.web_seed("hw1", 1))

    def test_prompts_format_with_the_reset_payload(self):
        ctx = {"base": "http://127.0.0.1:8792", "exp": th.reset_payload(7101), "work": "/w", "downloads": "/d",
               "ad": "/ad", "cdp_port": 1}
        for task in th.WEB_TASKS:
            text = task["prompt"].format(**ctx)
            self.assertNotIn("{", text, task["id"])

    def test_hw2_window_has_boundary_orders(self):
        for seed in range(7000, 7040):
            w = th.world(seed)["hw2"]
            start = datetime.date.fromisoformat(w["start"])
            dates = {o["date"] for o in w["orders"]}
            self.assertIn(start.isoformat(), dates)
            self.assertIn((start - datetime.timedelta(days=1)).isoformat(), dates)
            self.assertGreater(w["count"], 0)

    def test_hw4_newest_is_not_first_by_hot(self):
        for seed in range(7000, 7040):
            server.reset(seed)
            w = server.CACHE["w"]["hw4"]
            now = datetime.datetime.fromisoformat(w["now"])
            books = sorted((p for p in w["posts"] if p["forum"] == "books"), key=lambda p: server.hot_rank(p, now), reverse=True)
            self.assertNotEqual(books[0]["id"], w["target"], seed)

    def test_hw7_has_cheap_shoes_and_no_price_at_the_boundary(self):
        for seed in range(7000, 7040):
            w = th.world(seed)["hw7"]
            self.assertGreaterEqual(len(w["expected"]), 7)
            self.assertFalse(any(abs(p["price"] - 25) < 0.5 for p in w["products"] if p["category"] == "women-shoes"))


class LocalWebTests(unittest.TestCase):
    def setUp(self):
        self.seed = 7123
        server.reset(self.seed)
        self.w = th.world(self.seed)
        self.task = lambda tid: {"id": tid}
        self.ctx = {"exp": th.reset_payload(self.seed)}

    def verify(self, tid, answer=""):
        return th.verify_web(self.task(tid), self.ctx, answer, state())[0]

    def test_every_page_renders(self):
        w = self.w
        paths = ["/", "/shop/", "/shop/search?q=over-ear", "/shop/category/women-shoes/?price_max=25",
                 "/shop/product/10001?rp=2", "/shop/account/", "/shop/account/orders?p=2", "/admin/",
                 "/admin/catalog/products?search=" + w["hw3"]["color"], f"/admin/catalog/product/{w['hw3']['products'][0]['id']}/edit",
                 "/forum/", "/forum/f/books?sort=new", f"/forum/f/pics/{w['hw5']['post']['id']}", "/git/",
                 "/git/a11yproject/a11yproject.com", "/git/a11yproject/a11yproject.com/-/issues/new"]
        for p in paths:
            status, body, _ = get(p)
            self.assertEqual(status, 200, p)
            self.assertIn("<h1>", body, p)

    def test_hw1_names(self):
        exp = self.w["hw1"]["expected"]
        self.assertTrue(th.verify_hw1(self.w, ", ".join(exp), {})[0])
        self.assertFalse(th.verify_hw1(self.w, ", ".join(exp[:-1]), {})[0])
        self.assertFalse(th.verify_hw1(self.w, ", ".join(exp + [self.w["hw1"]["others"][0]]), {})[0])
        body = "".join(get(f"/shop/product/10001?rp={n}")[1] for n in range(1, 5))
        for name in exp:
            self.assertIn(name, body)

    def test_hw2_count_and_total(self):
        e = self.w["hw2"]
        self.assertTrue(th.verify_hw2(self.w, f"DONE {e['count']} | ${e['total']:,.2f}", {})[0])
        self.assertFalse(th.verify_hw2(self.w, f"DONE {e['count'] + 1} | ${e['total']:,.2f}", {})[0])
        self.assertFalse(th.verify_hw2(self.w, f"DONE {e['count']} | ${e['total'] + 1:,.2f}", {})[0])
        self.assertFalse(th.verify_hw2(self.w, f"DONE {e['count']} {e['total']}", {})[0])

    def test_hw3_price_edits_through_the_server(self):
        targets = self.w["hw3"]["targets"]
        self.assertEqual(len(targets), 5)
        self.assertFalse(self.verify("hw3"))
        for pid, price in targets.items():
            status, _, extra = post(f"/admin/catalog/product/{pid}/save", {"price": f"{price:.2f}"})
            self.assertEqual(status, 303)
        self.assertTrue(self.verify("hw3"))
        other = next(p for p in self.w["hw3"]["products"] if p["id"] not in targets)
        post(f"/admin/catalog/product/{other['id']}/save", {"price": f"{other['price'] - 5:.2f}"})
        self.assertFalse(self.verify("hw3"))

    def test_hw4_vote_toggles(self):
        target = self.w["hw4"]["target"]
        post("/forum/vote", {"post": target, "dir": "up", "back": "/forum/f/books"})
        self.assertTrue(self.verify("hw4"))
        post("/forum/vote", {"post": target, "dir": "up", "back": "/forum/f/books"})
        self.assertFalse(self.verify("hw4"))
        other = next(p["id"] for p in self.w["hw4"]["posts"] if p["id"] != target)
        post("/forum/vote", {"post": target, "dir": "up"})
        post("/forum/vote", {"post": other, "dir": "up"})
        self.assertFalse(self.verify("hw4"))

    def test_hw5_reply_under_the_moderator(self):
        t = self.w["hw5"]
        quoting = next(c["id"] for c in t["comments"] if c["body"].startswith("Quoting"))
        post("/forum/comment", {"post": t["post"]["id"], "parent": quoting, "body": t["reply"]})
        self.assertFalse(self.verify("hw5"))
        server.reset(self.seed)
        post("/forum/comment", {"post": t["post"]["id"], "parent": t["mod_comment"], "body": t["reply"]})
        self.assertTrue(self.verify("hw5"))
        self.assertIn("MOD", get(f"/forum/f/pics/{t['post']['id']}")[1])

    def test_hw6_issue_fields(self):
        g = self.w["hw6"]
        form = {"title": "401 bad gateway", "assignee": g["assignee"]["username"], "due": g["due"]}
        post("/git/a11yproject/a11yproject.com/-/issues", form)
        self.assertTrue(self.verify("hw6"))
        server.reset(self.seed)
        wrong = dict(form, due=g["due"][:-2] + "01")
        post("/git/a11yproject/a11yproject.com/-/issues", wrong)
        self.assertFalse(self.verify("hw6"))
        self.assertTrue(g["due"].endswith(str(__import__("calendar").monthrange(int(g["due"][:4]), int(g["due"][5:7]))[1])))

    def test_hw7_last_listing(self):
        get("/shop/category/women-shoes/?price_min=20&price_max=39.99")
        self.assertFalse(self.verify("hw7"))
        get("/shop/category/women-shoes/?price_min=&price_max=25&p=2")
        self.assertTrue(self.verify("hw7"))
        get("/shop/category/girls-shoes/?price_max=25")
        self.assertFalse(self.verify("hw7"))
        get("/shop/category/women-shoes/?price_max=24.99&sort=price")
        self.assertTrue(self.verify("hw7"))
        get("/shop/product/10001")
        self.assertFalse(self.verify("hw7"))


class PublicWebTests(unittest.TestCase):
    def test_arxiv_v3_date(self):
        page = (FIX / "arxiv_abs_snippet.html").read_text()
        self.assertEqual(th.parse_arxiv_version(page, 3), "2023-03-27")
        exp = {"hp1_v3": "2023-03-27"}
        self.assertTrue(th.verify_public("hp1", exp, "DONE v3 was submitted on Mon, 27 Mar 2023 17:46:54 UTC")[0])
        self.assertTrue(th.verify_public("hp1", exp, "DONE March 27, 2023")[0])
        self.assertFalse(th.verify_public("hp1", exp, "DONE 16 Mar 2023")[0])

    def test_arxiv_econ(self):
        econ = th.parse_arxiv_econ((FIX / "arxiv_taxonomy_snippet.html").read_text())
        self.assertEqual(econ, {"econ.EM": "Econometrics", "econ.GN": "General Economics", "econ.TH": "Theoretical Economics"})
        good = "DONE Econometrics (econ.EM), General Economics (econ.GN), Theoretical Economics (econ.TH)"
        self.assertTrue(th.verify_public("hp2", {"hp2_econ": econ}, good)[0])
        self.assertFalse(th.verify_public("hp2", {"hp2_econ": econ}, "DONE econ.EM, econ.GN")[0])

    def test_cambridge_count(self):
        n = th.parse_cambridge_meanings((FIX / "cambridge_unblemished_snippet.html").read_text())
        self.assertEqual(n, 2)
        self.assertTrue(th.verify_public("hp3", {"hp3_count": 2}, "DONE 2")[0])
        self.assertTrue(th.verify_public("hp3", {"hp3_count": 2}, "DONE two")[0])
        self.assertFalse(th.verify_public("hp3", {"hp3_count": 2}, "DONE 3")[0])


class NativeJudgeTests(unittest.TestCase):
    def test_subscript(self):
        want = [t.replace("{n}", "7") for t, _ in th.H2O_DOC]
        self.assertTrue(th.judge_subscript(fixture("h2o_good.json"), want)[0])
        self.assertFalse(th.judge_subscript(fixture("h2o_seed.json"), want)[0])
        self.assertFalse(th.judge_subscript(fixture("h2o_overdone.json"), want)[0])

    def test_strike(self):
        want = [t for t, _ in th.OUTLINE_DOC]
        self.assertTrue(th.judge_strike(fixture("strike_good.json"), want)[0])
        self.assertFalse(th.judge_strike(fixture("strike_seed.json"), want)[0])
        self.assertFalse(th.judge_strike(fixture("strike_all.json"), want)[0])

    def test_center(self):
        want = [t for t, _ in th.HEADING_DOC]
        self.assertTrue(th.judge_center(fixture("center_good.json"), want)[0])
        self.assertFalse(th.judge_center(fixture("center_seed.json"), want)[0])
        self.assertFalse(th.judge_center(fixture("center_all.json"), want)[0])

    def test_caps(self):
        want = [t for t, _ in th.CAPS_DOC]
        self.assertTrue(th.judge_caps(fixture("caps_good.json"), want)[0])
        self.assertFalse(th.judge_caps(fixture("caps_seed.json"), want)[0])

    def test_flip(self):
        self.assertTrue(th.judge_flip(fixture("img_flip.json"))[0])
        self.assertFalse(th.judge_flip(fixture("img_seed.json"))[0])
        self.assertFalse(th.judge_flip(None)[0])

    def test_resize(self):
        self.assertTrue(th.judge_resize(fixture("img_resized.json"))[0])
        self.assertFalse(th.judge_resize(fixture("img_squash.json"))[0])
        self.assertFalse(th.judge_resize(fixture("img_seed.json"))[0])

    def test_timezone(self):
        names = fixture("tz_berlin.json")
        self.assertTrue(th.judge_timezone("Central European Standard Time", "Europe/Berlin", names)[0])
        self.assertTrue(th.judge_timezone("Berlin - Germany", "Europe/Berlin", names)[0])
        self.assertFalse(th.judge_timezone("Pacific Standard Time", "Europe/Berlin", names)[0])

    def test_display_minutes(self):
        self.assertEqual(th.minutes_said("For 10 minutes"), 10)
        self.assertEqual(th.minutes_said("For 1 hour"), 60)
        self.assertEqual(th.minutes_said("Never"), 0)
        self.assertIsNone(th.minutes_said("I could not find it"))

    def test_seed_png_round_trip(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "x.png"
            th.write_png(p, 8, 4, lambda x, y: (x * 30, y * 60, 0))
            self.assertEqual(p.read_bytes()[:8], b"\x89PNG\r\n\x1a\n")

    def test_expected_grids_differ(self):
        self.assertNotEqual(th.expected_grid(lambda u, v: (u, v)), th.expected_grid(lambda u, v: (1 - u, v)))


class AuditTests(unittest.TestCase):
    def test_own_tool_and_pipes_pass(self):
        self.assertIsNone(common.audit_native("agent-desktop snapshot --app TextEdit | jq .data", "ad-14", th.FORBIDDEN))
        self.assertIsNone(common.audit_native("peekaboo see --app Preview --json | head -50", "peekaboo", th.FORBIDDEN))

    def test_other_tools_and_readers_fail(self):
        self.assertEqual(common.audit_native("peekaboo see --app TextEdit", "ad-14", th.FORBIDDEN), "peekaboo")
        self.assertEqual(common.audit_native("agent-desktop snapshot", "cua", th.FORBIDDEN), "agent-desktop")
        self.assertEqual(common.audit_native("pmset -g custom", "ad-14", th.FORBIDDEN), "pmset")
        self.assertEqual(common.audit_native("FOO=1 textutil -convert txt x.rtf", "peekaboo", th.FORBIDDEN), "textutil")

    def test_cat_only_for_the_guide(self):
        guide = common.NATIVE_GUIDE_DIR["peekaboo"] / "SKILL.md"
        self.assertIsNone(common.audit_native(f"cat {guide}", "peekaboo", th.FORBIDDEN))
        self.assertEqual(common.audit_native("cat ~/agent-desktop-bench/x.rtf", "peekaboo", th.FORBIDDEN), "cat")
        self.assertEqual(common.audit_native(f"cat {guide}", "ad-14", th.FORBIDDEN), "cat")


if __name__ == "__main__":
    unittest.main()
