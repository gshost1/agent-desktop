"""Arm definitions shared by run_native.py, run_web.py and report.py.

Each arm is one tool at one pinned version, reached through its wrapper in arms/bin/<arm>/, which
logs every call's wall time. Every arm gets the same task prompt; only the tool paragraph of the
system prompt differs, and it points the model at that tool's own guide (its skill or help).
"""
import importlib.util
import os
import re
import sys
from pathlib import Path

import shellwords

HERE = Path(__file__).resolve().parent
HELDOUT = HERE.parent
BENCH = HELDOUT.parent
TOOLS = Path.home() / ".agent-desktop-bench-target" / "heldout-tools"
SKILLS = TOOLS / "skills"
BROWSER_BENCH = Path.home() / ".t3/worktrees/agent-desktop/feat-unified-browser/bench/browser"
VERIFY_AD = TOOLS / "agent-desktop-1.4.0" / "agent-desktop"

TOOL_NAMES = {"agent-desktop", "peekaboo", "cua-driver", "browser-harness", "playwright-cli", "agent-browser"}

SCREEN_OFF = "Screen capture is off in this benchmark for every tool: read the accessibility tree instead ({off}). " \
    "A run whose tool captures the screen fails."
CAPTURE_OFF = {"agent-desktop": "no `screenshot` or `--screenshot`",
               "peekaboo": "pass `--no-screenshot` to `see`; no `image`",
               "cua-driver": "pass `include_screenshot:false` to every read, including `observe`; no `screenshot`"}

NATIVE_TOOL_TEXT = {
    "ad-up": ("You operate this Mac's GUI through the `agent-desktop` CLI (on PATH, accessibility granted). "
              "Learn it with `agent-desktop skills get desktop` (and `agent-desktop skills get platform`) before you start."),
    "ad-14": ("You operate this Mac's GUI through the `agent-desktop` CLI (on PATH, accessibility granted). "
              "Learn it with `agent-desktop skills get desktop` (and `agent-desktop skills get platform`) before you start."),
    "peekaboo": ("You operate this Mac's GUI through the `peekaboo` CLI (on PATH, accessibility granted). "
                 f"Learn it from its skill before you start: `cat {SKILLS}/peekaboo/SKILL.md`."),
    "cua": ("You operate this Mac's GUI through the `cua-driver` CLI (on PATH; tools run as `cua-driver call <tool> '<json>'`). "
            f"Learn it from its skill before you start: `cat {SKILLS}/cua-driver/SKILL.md` (its other guides, such as "
            "MACOS.md and WORKFLOW.md, are in the same folder)."),
}
NATIVE_TOOL = {"ad-up": "agent-desktop", "ad-14": "agent-desktop", "peekaboo": "peekaboo", "cua": "cua-driver"}
NATIVE_GUIDE_DIR = {"peekaboo": SKILLS / "peekaboo", "cua": SKILLS / "cua-driver"}

WEB_ARMS = {"ad-up": "ax", "ad-14": "ad-web", "ad-137": "ad-web", "bh": "cdp", "pw": "cdp", "ab": "cdp"}
WEB_TOOL = {"ad-up": "agent-desktop", "ad-14": "agent-desktop", "ad-137": "agent-desktop", "bh": "browser-harness", "pw": "playwright-cli",
            "ab": "agent-browser"}

ARM_VERSIONS = {
    "ad-up": "agent-desktop upstream 8e8dd7cbe (bench-upstream release build)",
    "ad-14": "agent-desktop 1.4.0 (fork release v1.4.0, aarch64-apple-darwin)",
    "ad-137": "agent-desktop 1.4.0 + fork PR #137 9e51c527d (release build, aarch64-apple-darwin)",
    "peekaboo": "Peekaboo 4.9.8 (openclaw/Peekaboo release tarball)",
    "cua": "Cua Driver 0.47.7 (trycua/cua cua-driver-rs-v0.47.7)",
    "bh": "browser-harness 0.1.13 (browser-use, bh-venv)",
    "pw": "@playwright/cli 0.1.24 (microsoft/playwright-cli)",
    "ab": "agent-browser 0.42.0 (vercel-labs/agent-browser, npm)",
}


CANDIDATES = TOOLS / "candidates"
CANDIDATE_ARMS = set()
CANDIDATE_WRAPPER = """#!/bin/zsh
# Candidate arm {label}: an agent-desktop build handed to the bench (see its VERSION file).
exec /bin/zsh "{logwrap}" "${{0:A:h:h}}/agent-desktop" "$@"
"""


def register_candidates(root=CANDIDATES):
    """Arms for the agent-desktop builds under heldout-tools/candidates/<label>/ (agent-desktop, its helper and
    a VERSION line). A candidate runs exactly like ad-14: same prompts, guide, web channel and audit. A label that
    names a built-in arm is ignored."""
    if not root.is_dir():
        return []
    labels = []
    for d in sorted(root.iterdir()):
        label = d.name
        if not (d / "agent-desktop").is_file() or (label in ARM_VERSIONS and label not in CANDIDATE_ARMS):
            continue
        NATIVE_TOOL_TEXT[label] = NATIVE_TOOL_TEXT["ad-14"]
        NATIVE_TOOL[label] = "agent-desktop"
        WEB_ARMS[label] = "ad-web"
        WEB_TOOL[label] = "agent-desktop"
        version = d / "VERSION"
        ARM_VERSIONS[label] = version.read_text().strip() if version.exists() else f"agent-desktop candidate {label}"
        CANDIDATE_ARMS.add(label)
        labels.append(label)
    return labels


def arm_bin_dir(arm, root=CANDIDATES):
    """The folder put first on PATH for an arm: arms/bin/<arm>, or a candidate's own bin/ with a logging wrapper."""
    if arm not in CANDIDATE_ARMS:
        return HERE / "bin" / arm
    bin_dir = root / arm / "bin"
    wrapper = bin_dir / "agent-desktop"
    text = CANDIDATE_WRAPPER.format(label=arm, logwrap=HERE / "logwrap.zsh")
    if not wrapper.exists() or wrapper.read_text() != text:
        bin_dir.mkdir(parents=True, exist_ok=True)
        wrapper.write_text(text)
        wrapper.chmod(0o755)
    return bin_dir


register_candidates()


RULES = os.environ.get("HO_RULES", "v2")
"""v1: the forbidden-program audit only. v2 (default): also run_audit.RunAudit and the background-only rule."""
FOREGROUND_TEXT = {"agent-desktop": "`--headed`, `focus-window` or `launch --activate`",
                   "peekaboo": "`--foreground`, `--web-focus`, `app switch`, `app launch` or `window focus`",
                   "cua-driver": "`bring_to_front`, `foreground_fallback` or a foreground delivery mode"}


def native_system_prompt(arm, screen_recording=False, allow_foreground=False):
    rules = ("Use Bash only to run the tool named above; piping its output through jq, grep or head is fine. Do not use "
             "any other program to do the task or to read the answer (no open, osascript, defaults, mkdir, ls, cat of anything but "
             "the tool's guide or output you saved, python, or any other GUI tool). ")
    if RULES == "v2":
        rules += ("Do not start another shell (sh, bash, eval), use the clipboard, or read files other than the tool's "
                  "guide and output you saved yourself. ")
        if not allow_foreground:
            rules += (f"Work in the background only: do not use {FOREGROUND_TEXT[NATIVE_TOOL[arm]]}, which take over "
                      "the shared screen, keyboard or mouse; a run that brings the task's app to the front fails "
                      "too. A run that breaks these rules fails. ")
    return NATIVE_TOOL_TEXT[arm] + " " + rules + ("" if screen_recording else SCREEN_OFF.format(off=CAPTURE_OFF[NATIVE_TOOL[arm]]) + " ") + \
        "When the task is complete, reply with the final answer only."


TASK_PREFIX = "Task: "


SPILL = re.compile(r"/\.claude/projects/[^/]+/[^/]+/tool-results/[^/]+$")


def _guide_or_spill(path, guide):
    """`cat` may read the arm's guide, or a file where Claude Code saved one of the model's own oversized tool
    results (it tells the model that path): that is the tool's output, not system state."""
    full = os.path.normpath(str(Path(path).expanduser()))
    return bool(guide and full.startswith(str(guide) + "/")) or bool(SPILL.search(full))


def audit_native(cmd, arm, forbidden, skills=None, check_cat=True):
    """The first program a Bash command runs that the arm may not use, or None.

    Forbidden: the bench's system-reading list, every other GUI tool, and `cat` outside the arm's guide folder.
    Commands are read by shellwords, so a `|` or `;` inside a quoted pattern never starts a new command.
    `skills` is the skills root the run saw (default: this host's), for re-auditing another host's rows.
    `check_cat=False` leaves `cat` to a caller that tracks what the run wrote (run_audit.RunAudit)."""
    own = NATIVE_TOOL[arm]
    guide = NATIVE_GUIDE_DIR.get(arm)
    if guide is not None and skills is not None:
        guide = Path(skills) / guide.name
    for words in shellwords.commands(cmd):
        prog = words[0].split("/")[-1]
        if prog == own:
            continue
        if prog == "cat" and check_cat:
            paths = [w for w in shellwords.drop_redirects(words[1:]) if not w.startswith("-")]
            if paths and all(_guide_or_spill(p, guide) for p in paths):
                continue
            return "cat"
        if prog != "cat" and (prog in TOOL_NAMES or prog in forbidden):
            return prog
    return None


def load_taskset():
    """The held-out task module: tasks_heldout, or the file HELDOUT_TASKSET names, which keeps the same contract
    (NATIVE_TASKS, WEB_TASKS, verify_web, ...). A private task set stays outside version control this way."""
    path = os.environ.get("HELDOUT_TASKSET")
    if not path:
        import tasks_heldout
        return tasks_heldout
    if "heldout_taskset" not in sys.modules:
        spec = importlib.util.spec_from_file_location("heldout_taskset", path)
        module = importlib.util.module_from_spec(spec)
        sys.modules["heldout_taskset"] = module
        spec.loader.exec_module(module)
    return sys.modules["heldout_taskset"]


def taskset_runs(th):
    """Where a task set's rows go: heldout/runs for tasks_heldout, heldout/runs/<NAME> for any other set."""
    name = getattr(th, "NAME", None)
    return HELDOUT / "runs" / name if name else HELDOUT / "runs"
