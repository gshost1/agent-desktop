"""A stand-in for the Messages API, for a dry run of the harness with no model and no spend.

python3 -I model_stub.py <port> <config.json>

Claude Code runs against it with ANTHROPIC_BASE_URL=http://127.0.0.1:<port> and a dummy key. Each request is
answered from the conversation so far: the first turn calls Bash with the arm tool's status command, the second
with one read of the task's app, the third ends the run with a text answer. The config (kept outside the
repository, because it names the task apps) is
  {"status": {"<tool>": "<command>"}, "reads": {"<app>": {"<tool>": "<command>"}}, "answer": "<text>"}
The tool is the first config tool named in the system prompt; the app is the first config app named in the first
user message. Every request's size, the commands sent, which auth header came (x-api-key or authorization) and the
first 12 hex digits of its sha256 are appended to <config>.log; never a header value.
"""
import hashlib
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PORT, CONFIG = int(sys.argv[1]), Path(sys.argv[2])
LOG = CONFIG.with_suffix(".log")


def text_of(content):
    if isinstance(content, str):
        return content
    return " ".join(b.get("text", "") for b in content or [] if isinstance(b, dict))


def system_text(body):
    s = body.get("system")
    return s if isinstance(s, str) else " ".join(b.get("text", "") for b in s or [] if isinstance(b, dict))


def plan(body):
    """(kind, payload): ("bash", command) or ("text", answer)."""
    cfg = json.loads(CONFIG.read_text())
    messages = body.get("messages") or []
    results = sum(1 for m in messages if m.get("role") == "user" and isinstance(m.get("content"), list)
                  for b in m["content"] if isinstance(b, dict) and b.get("type") == "tool_result")
    sys_text = system_text(body)
    tool = next((t for t in cfg["status"] if t in sys_text), None)
    first = text_of(messages[0].get("content")) if messages else ""
    app = next((a for a in cfg["reads"] if a in first), None)
    if tool is None or not body.get("tools"):
        return "text", cfg.get("answer", "stub")
    if results == 0:
        return "bash", cfg["status"][tool]
    if results == 1 and app is not None:
        return "bash", cfg["reads"][app][tool]
    return "text", cfg.get("answer", "stub")


def message(kind, payload, model):
    if kind == "bash":
        block = {"type": "tool_use", "id": f"toolu_stub{int(time.time() * 1000)}", "name": "Bash",
                 "input": {"command": payload, "description": "stub step"}}
        stop = "tool_use"
    else:
        block = {"type": "text", "text": payload}
        stop = "end_turn"
    return {"id": f"msg_stub{int(time.time() * 1000)}", "type": "message", "role": "assistant", "model": model,
            "content": [block], "stop_reason": stop, "stop_sequence": None,
            "usage": {"input_tokens": 10, "output_tokens": 5, "cache_creation_input_tokens": 0,
                      "cache_read_input_tokens": 0}}


def sse(msg):
    block = msg["content"][0]
    start = dict(msg, content=[], stop_reason=None)
    if block["type"] == "tool_use":
        first = dict(block, input={})
        delta = {"type": "input_json_delta", "partial_json": json.dumps(block["input"])}
    else:
        first = {"type": "text", "text": ""}
        delta = {"type": "text_delta", "text": block["text"]}
    events = [("message_start", {"type": "message_start", "message": start}),
              ("content_block_start", {"type": "content_block_start", "index": 0, "content_block": first}),
              ("content_block_delta", {"type": "content_block_delta", "index": 0, "delta": delta}),
              ("content_block_stop", {"type": "content_block_stop", "index": 0}),
              ("message_delta", {"type": "message_delta", "delta": {"stop_reason": msg["stop_reason"],
                                 "stop_sequence": None}, "usage": {"output_tokens": 5}}),
              ("message_stop", {"type": "message_stop"})]
    return "".join(f"event: {e}\ndata: {json.dumps(d)}\n\n" for e, d in events).encode()


def auth(headers):
    """Which credential the request carried, as a header name and a 12-digit hash prefix, never the value."""
    for name in ("x-api-key", "authorization"):
        value = headers.get(name)
        if value:
            return {"auth": name, "auth_sha12": hashlib.sha256(value.encode()).hexdigest()[:12]}
    return {"auth": None}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, code, data, ctype="application/json"):
        self.send_response(code)
        self.send_header("content-type", ctype)
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.reply(200, b"{}")

    def do_HEAD(self):
        self.reply(200, b"")

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("content-length") or 0))
        try:
            body = json.loads(raw or b"{}")
        except ValueError:
            body = {}
        path = self.path.split("?")[0]
        if path.endswith("/count_tokens"):
            return self.reply(200, b'{"input_tokens": 10}')
        if not path.endswith("/v1/messages"):
            return self.reply(200, b"{}")
        kind, payload = plan(body)
        with open(LOG, "a") as fh:
            fh.write(json.dumps({"t": round(time.time(), 3), "bytes": len(raw), "kind": kind,
                                 "command": payload if kind == "bash" else None, **auth(self.headers)}) + "\n")
        msg = message(kind, payload, body.get("model", "stub"))
        if body.get("stream"):
            return self.reply(200, sse(msg), "text/event-stream")
        return self.reply(200, json.dumps(msg).encode())


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
