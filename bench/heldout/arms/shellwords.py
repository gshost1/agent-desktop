"""The programs a Bash command line runs, for the forbidden-command audit.

A small shell reader, not a shell: it honours single and double quotes, backslash escapes, `$( )` and
backtick substitution (also inside double quotes), subshells, `|`, `||`, `&&`, `;`, `&` and newlines,
skips here-document bodies and comments, and drops assignments, redirections, shell keywords and
transparent prefixes (env, xargs, time, ...) before the program word. A `|` or `;` inside a quoted
grep pattern is part of the pattern, never a separator; the old regex split treated it as one.
"""
import re

KEYWORDS = {"if", "then", "else", "elif", "fi", "do", "done", "while", "until", "!", "{", "}", "esac"}
PREFIXES = {"time", "nohup", "command", "exec", "builtin", "nice", "caffeinate"}
FLAG_PREFIXES = {"env", "xargs", "sudo", "timeout", "gtimeout"}
NON_COMMANDS = {"for", "case", "select", "function"}
ASSIGNMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*\+?=")
REDIRECT = re.compile(r"^(\d*|&)(>>?|<<?<?|<>|>\|)(&\d*-?)?")
SUBST = "\x00"


class _Reader:
    def __init__(self, text):
        self.s = text
        self.commands = []
        self.heredocs = []

    def parse(self, i, end):
        words, word, started, quote = [], [], False, None
        expect_delim = False

        def flush_word():
            nonlocal word, started, expect_delim
            if started:
                text = "".join(word)
                if expect_delim:
                    self.heredocs.append(text)
                    expect_delim = False
                elif re.match(r"^\d*<<-?$", text):
                    expect_delim = True
                else:
                    m = re.match(r"^\d*<<-?(?!<)(.+)$", text)
                    if m:
                        self.heredocs.append(m.group(1))
                words.append(text)
            word, started = [], False

        def flush_command():
            flush_word()
            if words:
                self.commands.append(list(words))
            words.clear()

        s = self.s
        while i < len(s):
            c = s[i]
            if quote == "'":
                if c == "'":
                    quote = None
                else:
                    word.append(c)
                i += 1
                continue
            if c == "\\" and i + 1 < len(s):
                nxt = s[i + 1]
                if quote == '"' and nxt not in '"\\$`\n':
                    word.append(c)
                if nxt != "\n":
                    word.append(nxt)
                started = True
                i += 2
                continue
            if s.startswith("$((", i):
                close = s.find("))", i + 3)
                i = len(s) if close < 0 else close + 2
                word.append(SUBST)
                started = True
                continue
            if s.startswith("$(", i):
                i = self.parse(i + 2, ")")
                word.append(SUBST)
                started = True
                continue
            if c == "`":
                if end == "`":
                    flush_command()
                    return i + 1
                i = self.parse(i + 1, "`")
                word.append(SUBST)
                started = True
                continue
            if quote == '"':
                if c == '"':
                    quote = None
                else:
                    word.append(c)
                i += 1
                continue
            if c in "'\"":
                quote = c
                started = True
                i += 1
                continue
            if c == "#" and not started:
                close = s.find("\n", i)
                i = len(s) if close < 0 else close
                continue
            if c == ")" and end == ")":
                flush_command()
                return i + 1
            if c in " \t":
                flush_word()
                i += 1
                continue
            if c == "&" and (s.startswith("&>", i) or (word and word[-1] in "<>")):
                word.append(c)
                started = True
                i += 1
                continue
            if c == "(":
                flush_command()
                i = self.parse(i + 1, ")")
                continue
            if c in ";|&\n":
                flush_command()
                i += 1
                if c == "\n" and self.heredocs:
                    i = self.skip_heredocs(i)
                continue
            word.append(c)
            started = True
            i += 1
        flush_command()
        return i

    def skip_heredocs(self, i):
        s = self.s
        while self.heredocs:
            delim = self.heredocs.pop(0)
            strip = delim.startswith("-")
            delim = delim.lstrip("-")
            while i < len(s):
                close = s.find("\n", i)
                line = s[i:] if close < 0 else s[i:close]
                i = len(s) if close < 0 else close + 1
                if (line.strip() if strip else line) == delim:
                    break
        return i


def _program_words(words):
    """The words from the program on, or [] when the command runs no program."""
    out = list(words)
    while out:
        w = out[0]
        if ASSIGNMENT.match(w) or w in KEYWORDS or w in PREFIXES:
            out.pop(0)
            continue
        if REDIRECT.match(w):
            out.pop(0)
            if REDIRECT.fullmatch(w) and out:
                out.pop(0)
            continue
        if w in NON_COMMANDS:
            return []
        if w in FLAG_PREFIXES:
            out.pop(0)
            while out and (out[0].startswith("-") or ASSIGNMENT.match(out[0]) or (w == "timeout" and out[0][:1].isdigit())):
                out.pop(0)
            continue
        break
    return out


def drop_redirects(args):
    """Arguments without redirection operators and their targets."""
    out, skip = [], False
    for w in args:
        if skip:
            skip = False
            continue
        if REDIRECT.match(w):
            skip = bool(REDIRECT.fullmatch(w))
            continue
        out.append(w)
    return out


def commands(cmd):
    """Each simple command a Bash command line runs, as its words from the program word on."""
    reader = _Reader(cmd)
    reader.parse(0, None)
    for words in reader.commands:
        prog = _program_words(words)
        if prog:
            yield prog
