#!/usr/bin/env python3
"""Re-summarize a run whose driver crashed after the agent finished.

Usage: python3 bench/recover.py RUN_DIR TASK CONFIG SUCCESS VERIFY_NOTE [MODEL]
SUCCESS is true/false, judged by the caller from the run's leftover state.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from driver import ROOT, summarize  # noqa: E402
from tasks import audit_command  # noqa: E402

rdir, task, config, success, note = Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4] == "true", sys.argv[5]
model = sys.argv[6] if len(sys.argv) > 6 else "claude-opus-5-5"
events = [json.loads(line) for line in (rdir / "stream.jsonl").read_text().splitlines()]
s = summarize(events, rdir, events[-1]["_t"])
forbidden = [f for f in (audit_command(c) for c in s.pop("commands")) if f]
s.update({"run_id": rdir.name, "task": task, "config": config, "model": model, "effort": "default",
          "success": success and not forbidden, "verify": note + " (recovered)"})
(rdir / "summary.json").write_text(json.dumps(s, indent=1))
with open(ROOT / "runs" / "results.jsonl", "a") as fh:
    fh.write(json.dumps(s) + "\n")
print(rdir.name, task, config, s["success"], s["wall_s"], s["api_calls"], s["input_total"], s["cli_s"])
