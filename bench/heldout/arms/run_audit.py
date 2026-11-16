"""Whole-run audit of a native arm's Bash commands, read in order from the run's full stream.

On top of common.audit_native (the forbidden programs, other GUI tools, and `cat` outside the guide), a run fails
when a command:
  - runs another shell or an indirection that hides a program (sh, bash, zsh, eval, source, ...);
  - uses the clipboard (pbpaste, pbcopy);
  - reads a file with a text tool (head, tail, sed, grep, awk, jq, wc, strings, ...) or a `<` redirect, unless the
    file is the arm's guide, a Claude Code spill file, or a file this run wrote earlier (`tool ... > /tmp/s.json`
    and then `grep x /tmp/s.json` only reads the tool's own output);
  - uses its own tool's clipboard (agent-desktop `clipboard-*`, peekaboo `clipboard`/`paste`, cua clipboard tools);
  - drives the UI in the foreground when the task does not allow it: agent-desktop `--headed`, `focus-window` or
    `launch --activate`; peekaboo `--foreground`, `--web-focus`, `app switch|launch` or `window focus`; cua-driver
    `bring_to_front`, a foreground delivery mode or `foreground_fallback`;
  - writes anywhere but /dev/null, a temp folder or its own working folder (`>`, `>>`, `tee`);
  - runs any program outside a fixed set (its tool, the text tools, a few shell builtins, functions and literal
    variables the run defined itself). An unrecognised form fails closed instead of passing unread.
Text tools that can read a file from inside their program fail closed too: awk `getline`/`system`/pipes, sed
`r`/`w`/`e` commands, jq `import`/`include`/`$ENV`/`env`, and jq `--rawfile`/`--slurpfile` name a file that must be
readable. Paths are normalised before the guide and spill checks, so `guide/../x` is outside the guide.

A path given through a shell variable (`"$F"`) cannot be resolved here and is refused, because it can name any
file; the tool's own refs (`$R`) are not file arguments of a text tool and never reach this check.

What a lexical audit cannot see (a background call that activates its app anyway) is measured instead: run_native
samples the frontmost app during the run and fails a background-only run whose task app came to the front.
"""
import os
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
BUILTINS = {"cat", "echo", "printf", "sleep", "seq", "test", "[", "[[", "true", "false", ":", "break", "continue",
            "exit", "return", "set", "cd", "tr", "export", "local", "read", "shift", "unset", "wait", "let", "expr"}
FOREGROUND = {
    "agent-desktop": re.compile(r"(?<![\w-])--headed\b|^focus-window\b|^launch\b.*(?<![\w-])--activate\b"),
    "peekaboo": re.compile(r"(?<![\w-])--(?:foreground|web-focus)\b|^(?:app (?:switch|launch)|window focus)\b"),
    "cua-driver": re.compile(r"\bbring_to_front\b|delivery_mode\W{1,6}foreground|\"foreground\"\s*:\s*true"
                             r"|foreground_fallback\W{1,6}true"),
}
CLIPBOARD_OWN = {
    "agent-desktop": re.compile(r"^clipboard-"),
    "peekaboo": re.compile(r"^(?:clipboard|paste)\b"),
    "cua-driver": re.compile(r"\bclipboard_(?:read|write)\b|\bclipboard\s*\("),
}
PROGRAM_READS = {
    "awk": re.compile(r"\bgetline\b|\bsystem\s*\(|\bprintf?\b[^;{}]*\||\|&"),
    "sed": re.compile(r"(?:^|[;{}\n])\s*(?:(?:\d+|\$|/(?:[^/\\]|\\.)*/)(?:,(?:\d+|\$|/(?:[^/\\]|\\.)*/))?)?!?\s*[rRwWe](?:\s|$)"
                      r"|^s(.)(?:(?!\1).)*\1(?:(?!\1).)*\1[gpiI0-9]*[we]"),
    "jq": re.compile(r"\b(?:import|include|env|input_filename)\b|\$ENV|\$__prog"),
}
FILE_VALUE_OPTIONS = {"jq": {"--rawfile", "--slurpfile"}, "awk": {"-f"}}
ASSIGN = re.compile(r"(?<![\w$])([A-Za-z_]\w*)=(\"[^\"$`]*\"|'[^']*'|[^\s;&|$`'\"()<>]*)(?=[\s;&|)]|$)|(?<![\w$])([A-Za-z_]\w*)=")
FUNCTION = re.compile(r"(?:^|[;&|\n{(]|\bthen|\bdo)\s*(?:function\s+)?([A-Za-z_][\w-]*)\s*\(\s*\)|\bfunction\s+([A-Za-z_][\w-]*)")
HELP = {"--help", "-h", "help"}
STRING = re.compile(r'"(?:[^"\\]|\\.)*"')
VARIABLE_PROGRAM = re.compile(r"^\$\{?([A-Za-z_]\w*)\}?$")
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


def program_text(prog, args):
    """The inline program of a pattern tool: its first operand, or every `-e` value."""
    values = VALUE_OPTIONS.get(prog, set())
    scripts, skip, prev, first = [], 0, "", None
    for w in args:
        if skip:
            skip -= 1
            if prev in ("-e", "--regexp"):
                scripts.append(w)
            continue
        prev = w
        if w in values:
            skip = 2 if w in TWO_VALUE_OPTIONS else 1
            continue
        if w.startswith("-") and w not in STDIN_NAMES:
            continue
        if first is None:
            first = w
    return scripts or ([first] if first is not None else [])


def value_files(prog, args):
    """Files an option of `prog` reads: jq `--rawfile name f`, `--slurpfile name f`, awk `-f f`."""
    opts = FILE_VALUE_OPTIONS.get(prog, set())
    out = []
    for i, w in enumerate(args):
        if w in opts:
            j = i + (2 if w in TWO_VALUE_OPTIONS else 1)
            if j < len(args):
                out.append(args[j])
    return out


SCRATCH = ("/tmp/", "/private/tmp/", "/var/folders/", "$TMPDIR/", "${TMPDIR}/")


def writable(path):
    """A run may write only scratch output: /dev/null and the standard streams, a temp folder, or a relative path
    in its own empty working folder. A write anywhere else (a task file, the guide, a preference) fails."""
    if path in ("/dev/null", "/dev/stdout", "/dev/stderr") or re.fullmatch(r"&?\d*-?", path):
        return True
    if path.startswith(("$TMPDIR/", "${TMPDIR}/")):
        return "/../" not in path and "$" not in path.split("/", 1)[1]
    if "$" in path or "`" in path or "\x00" in path:
        return False
    norm = os.path.normpath(path)
    if norm.startswith("/"):
        return norm.startswith(tuple(p.rstrip("/") + "/" for p in SCRATCH[:3]))
    return not norm.startswith(("..", "~"))


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
    """The audit state of one run: files it wrote, functions and variables it defined, and whether its shell sits
    in the guide or spill folder."""

    def __init__(self, arm, forbidden, allow_foreground=False, skills=None):
        self.arm = arm
        self.forbidden = forbidden
        self.allow_foreground = allow_foreground
        self.skills = skills
        guide = common.NATIVE_GUIDE_DIR.get(arm)
        self.guide = Path(skills) / guide.name if guide is not None and skills is not None else guide
        self.written = set()
        self.functions = set()
        self.variables = {}
        self.cwd_readable = False

    def readable(self, path):
        if "$" in path or "`" in path or "\x00" in path:
            return False
        path = os.path.normpath(path)
        if path in self.written or common._guide_or_spill(path, self.guide):
            return True
        if "/heldout-tools/skills/" in path and self.guide is not None and f"/skills/{self.guide.name}/" in path:
            return True
        return self.cwd_readable and not path.startswith(("/", "~", ".."))

    def remember(self, cmd):
        """Functions and literal-valued variables a command defines; a non-literal assignment forgets the name."""
        for m in FUNCTION.finditer(cmd):
            self.functions.add(m.group(1) or m.group(2))
        for m in ASSIGN.finditer(cmd):
            if m.group(1):
                self.variables[m.group(1)] = m.group(2).strip("\"'").split("/")[-1]
            else:
                self.variables.pop(m.group(3), None)

    def program(self, word):
        m = VARIABLE_PROGRAM.match(word)
        if m:
            return self.variables.get(m.group(1), word)
        return word.split("/")[-1]

    def check(self, cmd):
        """The first violation in one Bash command, or None. Call it for every command of the run in order."""
        hit = common.audit_native(cmd, self.arm, self.forbidden, self.skills, check_cat=False)
        if hit:
            return hit
        self.remember(cmd)
        own = common.NATIVE_TOOL[self.arm]
        for words in shellwords.commands(cmd):
            prog = self.program(words[0])
            args, outs, ins = split_redirects(words[1:])
            if prog == "cd":
                target = os.path.normpath(args[0]) if args and "$" not in args[0] else ""
                self.cwd_readable = bool(re.search(r"tool-results$|/heldout-tools/skills/[^/]+$", target))
            if prog in SHELLS:
                return f"shell {prog}"
            if prog in CLIPBOARD:
                return prog
            if prog != "cat" and (prog in self.forbidden or prog in common.TOOL_NAMES and prog != own):
                return prog
            if prog not in BUILTINS | ALL_ARGS_READERS | PATTERN_READERS | self.functions | {own}:
                return f"unrecognised program {prog[:40]}"
            if prog == "sed" and any(w.startswith("-i") or w == "--in-place" for w in args):
                return "sed -i edits a file"
            if prog in PROGRAM_READS and any(PROGRAM_READS[prog].search(STRING.sub('""', t))
                                             for t in program_text(prog, args)):
                return f"{prog} program reads or runs outside its input"
            if prog in ALL_ARGS_READERS | PATTERN_READERS | {"cat"}:
                files = file_args(prog, args) + value_files(prog, args)
                if prog in WRITERS:
                    self.written.update(os.path.normpath(f) for f in files if "$" not in f)
                    files = []
                for f in files:
                    if not self.readable(f):
                        return f"{prog} reads {f[:80]}"
            for src in ins:
                if not self.readable(src):
                    return f"< {src[:80]}"
            if prog == own and not HELP.intersection(args):
                text = " ".join(args)
                if CLIPBOARD_OWN[own].search(text):
                    return f"clipboard {CLIPBOARD_OWN[own].search(text).group(0)}"
                if not self.allow_foreground and FOREGROUND[own].search(text):
                    return f"foreground {FOREGROUND[own].search(text).group(0)}"
            targets = outs + (file_args(prog, args) if prog in WRITERS else [])
            for t in targets:
                if not writable(t):
                    return f"writes {t[:80]}"
            self.written.update(os.path.normpath(t) for t in outs if "$" not in t and t not in STDIN_NAMES)
        return None
