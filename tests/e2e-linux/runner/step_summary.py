"""Render a live harness report.txt as Markdown for a CI step summary."""
from __future__ import annotations

import re
import sys
from pathlib import Path

PASSING = {"pass", "asserted-refusal"}
LEG = re.compile(r"^    (\S+)\s+(\S+)(?:  -- (.*))?$")
REASON_LIMIT = 240


def _cell(text: str) -> str:
    text = " ".join(text.split())
    if len(text) > REASON_LIMIT:
        text = text[: REASON_LIMIT - 1] + "…"
    return text.replace("|", "\\|").replace("`", "'")


def summarize(report: str) -> str:
    tier = mode = ""
    summary = None
    failing = []
    for line in report.splitlines():
        if line.startswith("tier "):
            tier = line[5:].strip()
        elif line.startswith("summary:"):
            summary = line[len("summary:"):].strip()
        elif line.startswith("  ") and not line.startswith("   "):
            mode = line.strip()
        else:
            match = LEG.match(line)
            if match and match[1] not in PASSING:
                failing.append((tier, mode, match[2], match[1], match[3] or ""))
    if summary is None:
        return no_report("report.txt has no summary line")
    lines = ["## Linux live e2e", "", f"**{summary}**", ""]
    if not failing:
        lines.append("Every leg passed.")
        return "\n".join(lines) + "\n"
    lines += ["| tier | mode | leg | outcome | reason |", "|---|---|---|---|---|"]
    for tier_name, mode_name, leg, outcome, reason in failing:
        lines.append(f"| {tier_name} | {mode_name} | `{leg}` | {outcome} | {_cell(reason)} |")
    return "\n".join(lines) + "\n"


def no_report(reason: str) -> str:
    return ("## Linux live e2e\n\n"
            f"**No leg results: {reason}.** The harness stopped before it wrote its report; "
            "read the harness step log.\n")


def main(argv: list[str]) -> int:
    reports = [Path(arg) for arg in argv if Path(arg).is_file()]
    if not reports:
        sys.stdout.write(no_report("no report.txt was written"))
        return 0
    for path in reports:
        sys.stdout.write(summarize(path.read_text(errors="replace")))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
