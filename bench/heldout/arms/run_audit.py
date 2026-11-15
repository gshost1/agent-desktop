"""Whole-run audit of a native arm's Bash commands, read in order from the run's full stream.

On top of common.audit_native (the forbidden programs, other GUI tools, and `cat` outside the guide), a run fails
when a command:
  - runs another shell or an indirection that hides a program (sh, bash, zsh, eval, source, ...);
  - uses the clipboard (pbpaste, pbcopy);
  - reads a file with a text tool (head, tail, sed, grep, awk, jq, wc, strings, ...) or a `<` redirect, unless the
    file is the arm's guide, a Claude Code spill file, or a file this run wrote earlier (`tool ... > /tmp/s.json`
    and then `grep x /tmp/s.json` only reads the tool's own output);
  - drives the UI in the foreground when the task does not allow it: agent-desktop `--headed`, peekaboo
    `--foreground`, cua-driver `bring_to_front` or a foreground delivery mode.

A path given through a shell variable (`"$F"`) cannot be resolved here and is refused, because it can name any
file; the tool's own refs (`$R`) are not file arguments of a text tool and never reach this check.
"""
import re
from pathlib import Path

import common
import shellwords

SHELLS = {"sh", "bash", "zsh", "dash", "ksh", "fish", "csh", "tcsh", "eval", "source", "."}
CLIPBOARD = {"pbpaste", "pbcopy"}
ALL_ARGS_READERS = {"head", "tail", "wc", "strings", "xxd", "od", "hexdump", "cut", "sort", "uniq", "less", "more",
                    "base64", "iconv", "file", "nl", "tac", "rev", "column", "paste", "comm", "diff", "cmp", "md5",
                    "shasum", "fold", "fmt", "expand", "join", "split", "tee"}
WRITERS = {"tee", "split"}
PATTERN_READERS = {"grep", "egrep", "fgrep", "rg", "sed", "awk", "jq"}
VALUE_OPTIONS = {
    "head": {"-n", "-c"}, "tail": {"-n", "-c"}, "cut": {"-d", "-f", "-c", "-b"}, "sort": {"-k", "-t", "-o"},
    "grep": {"-e", "-f", "-m", "-A", "-B", "-C", "-d", "-D", "--max-count", "--regexp", "--file"},
    "egrep": {"-e", "-f", "-m", "-A", "-B", "-C"}, "fgrep": {"-e", "-f", "-m", "-A", "-B", "-C"},
    "rg": {"-e", "-f", "-m", "-A", "-B", "-C", "-g", "-t"}, "sed": {"-e", "-f"}, "awk": {"-F", "-v", "-f"},
    "jq": {"--arg", "--argjson", "--slurpfile", "--rawfile", "--indent", "-L"},
    "fold": {"-w"}, "column": {"-s", "-t"}, "split": {"-l", "-b"}, "od": {"-t", "-N", "-j"},
    "xxd": {"-l", "-s", "-c"},
}
TWO_VALUE_OPTIONS = {"--arg", "--argjson", "--slurpfile", "--rawfile"}
SCRIPT_OPTIONS = {"-e", "-f", "--regexp", "--file"}
FOREGROUND = {
    "agent-desktop": re.compile(r"(?<![\w-])--headed\b"),
    "peekaboo": re.compile(r"(?<![\w-])--foreground\b"),
    "cua-driver": re.compile(r"\bbring_to_front\b|delivery_mode\W{1,6}foreground|\"foreground\"\s*:\s*true"),
}
OUTPUT_REDIRECT = re.compile(r"^\d*(>>?|>\|)(.*)$")
INPUT_REDIRECT = re.compile(r"^\d*<(?![<>])(.*)$")
STDIN_NAMES = {"-", "/dev/null", "/dev/stdin"}


def file_args(prog, args):
    """The file operands of a text tool, after its options and, for pattern tools, its pattern or program."""
    values = VALUE_OPTIONS.get(prog, set())
    files, scripts, skip, has_script, prev = [], [], 0, False, ""
    for w in args:
        if skip:
            skip -= 1
            if prev in ("-f", "--file") and prog != "awk":
                scripts.append(w)
            continue
        prev = w
        if w in values:
            skip = 2 if w in TWO_VALUE_OPTIONS else 1
            has_script = has_script or w in SCRIPT_OPTIONS
            continue
        if w.startswith("-") and w not in STDIN_NAMES:
            continue
        files.append(w)
    if prog in PATTERN_READERS and not has_script and files:
        files = files[1:]
    return [f for f in files + scripts if f not in STDIN_NAMES]


def split_redirects(args):
    """(arguments, output targets, input sources) of one simple command."""
    rest, outs, ins = [], [], []
    pending = None
    for w in args:
        if pending is not None:
            pending.append(w)
            pending = None
            continue
        if re.match(r"^\d*<<", w):
            if re.fullmatch(r"\d*<<[<-]?", w):
                pending = []
            continue
        m_out, m_in = OUTPUT_REDIRECT.match(w), INPUT_REDIRECT.match(w)
        if m_out and not w.startswith("&"):
            if m_out.group(2).startswith("&"):
                continue
            if m_out.group(2):
                outs.append(m_out.group(2))
            else:
                pending = outs
            continue
        if m_in:
            if m_in.group(1):
                ins.append(m_in.group(1))
            else:
                pending = ins
            continue
        rest.append(w)
    return rest, outs, ins


class RunAudit:
    """The audit state of one run: files it wrote, and whether its shell sits in the guide or spill folder."""

    def __init__(self, arm, forbidden, allow_foreground=False, skills=None):
        self.arm = arm
        self.forbidden = forbidden
        self.allow_foreground = allow_foreground
        self.skills = skills
        guide = common.NATIVE_GUIDE_DIR.get(arm)
        self.guide = Path(skills) / guide.name if guide is not None and skills is not None else guide
        self.written = set()
        self.cwd_readable = False

    def readable(self, path):
        if "$" in path or "`" in path:
            return False
        if path in self.written or common._guide_or_spill(path, self.guide):
            return True
        if "/heldout-tools/skills/" in path and self.guide is not None and f"/skills/{self.guide.name}/" in path:
            return True
        return self.cwd_readable and not path.startswith(("/", "~"))

    def check(self, cmd):
        """The first violation in one Bash command, or None. Call it for every command of the run in order."""
        hit = common.audit_native(cmd, self.arm, self.forbidden, self.skills)
        if hit:
            return hit
        own = common.NATIVE_TOOL[self.arm]
        for words in shellwords.commands(cmd):
            prog = words[0].split("/")[-1]
            args, outs, ins = split_redirects(words[1:])
            if prog == "cd":
                target = args[0] if args else ""
                self.cwd_readable = bool(re.search(r"tool-results/?$|/heldout-tools/skills/[^/]+/?$", target))
            if prog in SHELLS:
                return f"shell {prog}"
            if prog in CLIPBOARD:
                return prog
            if prog == "sed" and any(w.startswith("-i") or w == "--in-place" for w in args):
                return "sed -i edits a file"
            if prog in ALL_ARGS_READERS | PATTERN_READERS:
                files = file_args(prog, args)
                if prog in WRITERS:
                    self.written.update(f for f in files if "$" not in f)
                    files = []
                for f in files:
                    if not self.readable(f):
                        return f"{prog} reads {f[:80]}"
            for src in ins:
                if not self.readable(src):
                    return f"< {src[:80]}"
            if prog == own and not self.allow_foreground and FOREGROUND[own].search(" ".join(args)):
                return f"foreground {FOREGROUND[own].search(' '.join(args)).group(0)}"
            self.written.update(t for t in outs if "$" not in t and t not in STDIN_NAMES)
        return None
