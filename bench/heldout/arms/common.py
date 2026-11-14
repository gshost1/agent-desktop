"""Arm definitions shared by run_native.py, run_web.py and report.py.

Each arm is one tool at one pinned version, reached through its wrapper in arms/bin/<arm>/, which
logs every call's wall time. Every arm gets the same task prompt; only the tool paragraph of the
system prompt differs, and it points the model at that tool's own guide (its skill or help).
"""
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

SCREEN_OFF = "Screen Recording is off, so screenshots fail."

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


def native_system_prompt(arm, screen_recording=False):
    rules = ("Use Bash only to run the tool named above; piping its output through jq, grep or head is fine. Do not use "
             "any other program to do the task or to read the answer (no open, osascript, defaults, mkdir, ls, cat other "
             "than the tool's guide, python, or any other GUI tool). ")
    return NATIVE_TOOL_TEXT[arm] + " " + rules + ("" if screen_recording else SCREEN_OFF + " ") + \
        "When the task is complete, reply with the final answer only."


TASK_PREFIX = "Task: "


def audit_native(cmd, arm, forbidden, skills=None):
    """The first program a Bash command runs that the arm may not use, or None.

    Forbidden: the bench's system-reading list, every other GUI tool, and `cat` outside the arm's guide folder.
    Commands are read by shellwords, so a `|` or `;` inside a quoted pattern never starts a new command.
    `skills` is the skills root the run saw (default: this host's), for re-auditing another host's rows."""
    own = NATIVE_TOOL[arm]
    guide = NATIVE_GUIDE_DIR.get(arm)
    if guide is not None and skills is not None:
        guide = Path(skills) / guide.name
    for words in shellwords.commands(cmd):
        prog = words[0].split("/")[-1]
        if prog == own:
            continue
        if prog == "cat":
            paths = [w for w in shellwords.drop_redirects(words[1:]) if not w.startswith("-")]
            if guide and paths and all(str(Path(p).expanduser()).startswith(str(guide) + "/") for p in paths):
                continue
            return "cat"
        if prog in TOOL_NAMES or prog in forbidden:
            return prog
    return None
