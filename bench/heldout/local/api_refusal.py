"""Exit 1 when a transcript written since <marker> shows the API refusing the bench key.

Looks only at Claude Code's own error fields (an assistant event's `error`, a result event with
`is_error`), never at tool output, so page text such as "billing address" cannot trip it.
Prints the matching file and error kind, never request content.

Usage: python3 -I api_refusal.py <runs dir> <marker file>
"""
import json
import re
import sys
from pathlib import Path

REFUSAL = re.compile(r"billing|credit balance|spend(ing)? limit|usage limit|quota|insufficient|"
                     r"authentication|invalid x-api-key|api key|permission_error|organization.*disabled", re.I)


def refusal_in(path):
    for line in path.read_text(errors="replace").splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if not isinstance(ev, dict):
            continue
        if ev.get("type") == "assistant" and ev.get("error"):
            kind = str(ev.get("error"))
            text = json.dumps(ev.get("message", {}).get("content", ""))[:400]
            if REFUSAL.search(kind) or REFUSAL.search(text):
                return f"assistant error {kind}"
        if ev.get("type") == "result" and ev.get("is_error"):
            text = str(ev.get("result", ""))[:400]
            if text.startswith("API Error") and REFUSAL.search(text):
                status = re.search(r"API Error: (\d+)", text)
                return f"result API Error {status.group(1) if status else '?'}"
    return None


def main():
    runs, marker = Path(sys.argv[1]), Path(sys.argv[2])
    since = marker.stat().st_mtime if marker.exists() else 0
    for path in list(runs.glob("native/*/stream.jsonl")) + list(runs.glob("web/*/raw/*.jsonl")):
        if path.stat().st_mtime >= since:
            hit = refusal_in(path)
            if hit:
                print(f"{path.relative_to(runs)}: {hit}")
                sys.exit(1)


if __name__ == "__main__":
    main()
