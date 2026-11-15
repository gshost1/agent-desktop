#!/usr/bin/env python3
"""Fails when an error that may already have been delivered tells the caller to retry.

`recovery.strategy` and retry wording belong only to errors whose
`disposition.retry` is `safe`. An error built with a
`delivered_unverified` or `uncertain` disposition may already have reached
the application, so repeating the command blind can apply it twice. Its
suggestion must tell the caller to observe the result first.

The scan reads every non-test `.rs` file under `crates/` and `src/`. For each
`DeliverySemantics::delivered_unverified()` or `DeliverySemantics::uncertain()`
passed to `.with_disposition(...)`, and for each `DeliveryTracker::uncertain(...)`
wrapper, it collects the string literals of every `.with_suggestion(...)` in
the same builder chain, resolving a bare identifier to a `const` string of the
same file. A function whose body builds such a chain from one of its own
parameters, as `pressed_without_effect(message, suggestion)` does, makes every
call site's argument in that position a suggestion too, in any file. A
suggestion that says retry, again or rerun fails unless it also
says to look first: "before ...", "only if/when ...", "if it is still ...",
"do not ...", "instead of retrying" or "no automatic retry".

It cannot see a suggestion passed in as a parameter or a disposition chosen at
run time; those sites need their own unit tests.

It also reads every skill page under `skills/`. A passage (a table row, a list
item or a paragraph) that names, in backticks, a `details.kind` some delivered
or uncertain chain stamps, and advises a retry without a look-first step,
fails. Kinds come from the chains the Rust scan reads plus
`RUNTIME_DELIVERED_KINDS`, which lists the kinds whose disposition is chosen
in a variable or whose kind is not a literal in the chain. The skill scan
does not see a kind named without backticks, retry advice in a different
passage from the kind it concerns, or advice whose passage carries an
unrelated look-first word such as "before".

Run with `--self-test` to check the scanner against planted cases.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RETRY_WORDS = re.compile(r"\b(retry|retrying|again|rerun|re-run)\b", re.IGNORECASE)
LOOK_FIRST = re.compile(
    r"\bbefore\b|\bonly (if|when)\b|\bif (it|the entry) is still\b|\bdo not\b"
    r"|\binstead of retrying\b|\bno automatic retry\b",
    re.IGNORECASE,
)
OBSERVED_AGAIN = re.compile(
    r"\b(find|read|list|snapshot|look at)\b[^.;:,]*?\bagain\b", re.IGNORECASE
)
DISPOSITION = re.compile(r"^(delivered_unverified|uncertain)$")
OPEN = {"(": ")", "[": "]", "{": "}"}
CLOSE = {")", "]", "}"}
CHAIN_STOP = {";", ",", "=", "=>", "{", "}"}


def tokenize(text: str) -> list[tuple[str, str]]:
    """Splits Rust source into (kind, value) tokens: str, ident, punct."""
    tokens: list[tuple[str, str]] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            i = n if end < 0 else end
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            i = n if end < 0 else end + 2
        elif c == "r" and re.match(r'r#*"', text[i:]):
            hashes = re.match(r"r(#*)\"", text[i:]).group(1)
            start = i + 2 + len(hashes)
            end = text.find('"' + hashes, start)
            end = n if end < 0 else end
            tokens.append(("str", text[start:end]))
            i = end + 1 + len(hashes)
        elif c == '"' or (c == "b" and text.startswith('b"', i)):
            j = i + (2 if c == "b" else 1)
            buf = []
            while j < n and text[j] != '"':
                if text[j] == "\\" and j + 1 < n:
                    if text[j + 1] == "\n":
                        j += 2
                        while j < n and text[j] in " \t":
                            j += 1
                        continue
                    buf.append(text[j : j + 2])
                    j += 2
                    continue
                buf.append(text[j])
                j += 1
            tokens.append(("str", "".join(buf)))
            i = j + 1
        elif c == "'":
            m = re.match(r"'(\\.|[^\\'])'", text[i:])
            if m:
                tokens.append(("char", m.group(0)))
                i += len(m.group(0))
            else:
                m = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", text[i:])
                i += len(m.group(0)) if m else 1
        elif c.isalpha() or c == "_":
            m = re.match(r"[A-Za-z_][A-Za-z0-9_]*", text[i:])
            tokens.append(("ident", m.group(0)))
            i += len(m.group(0))
        elif c.isdigit():
            m = re.match(r"[0-9][0-9A-Za-z_.]*", text[i:])
            i += len(m.group(0))
        elif text.startswith("=>", i) or text.startswith("::", i):
            tokens.append(("punct", text[i : i + 2]))
            i += 2
        else:
            tokens.append(("punct", c))
            i += 1
    return tokens


def depths(tokens: list[tuple[str, str]]) -> list[int]:
    """The bracket depth each token sits at; an opener has its outer depth."""
    out, depth = [], 0
    for kind, value in tokens:
        if kind == "punct" and value in CLOSE:
            depth -= 1
        out.append(depth)
        if kind == "punct" and value in OPEN:
            depth += 1
    return out


def drop_test_items(tokens: list[tuple[str, str]]) -> list[tuple[str, str]]:
    """Removes every item gated by `#[cfg(test)]`, up to its `;` or closing brace."""
    out: list[tuple[str, str]] = []
    i, n = 0, len(tokens)
    while i < n:
        if (
            tokens[i] == ("punct", "#")
            and tokens[i + 1 : i + 6]
            == [("punct", "["), ("ident", "cfg"), ("punct", "("), ("ident", "test"), ("punct", ")")]
        ):
            j, depth = i, 0
            while j < n:
                kind, value = tokens[j]
                if kind == "punct" and value in OPEN:
                    depth += 1
                elif kind == "punct" and value in CLOSE:
                    depth -= 1
                    if depth == 0 and value == "}":
                        break
                elif kind == "punct" and value == ";" and depth == 0:
                    break
                j += 1
            i = j + 1
            continue
        out.append(tokens[i])
        i += 1
    return out


def string_consts(tokens: list[tuple[str, str]]) -> dict[str, str]:
    consts = {}
    for i in range(len(tokens) - 6):
        if tokens[i] == ("ident", "const") and tokens[i + 1][0] == "ident":
            j = i + 2
            while j < len(tokens) and tokens[j] != ("punct", "="):
                j += 1
            if j + 1 < len(tokens) and tokens[j + 1][0] == "str":
                consts[tokens[i + 1][1]] = tokens[j + 1][1]
    return consts


def matching_close(tokens, depth, start):
    """Index of the token closing the group opened at `start`."""
    level = depth[start]
    for k in range(start + 1, len(tokens)):
        if depth[k] == level and tokens[k][0] == "punct" and tokens[k][1] in CLOSE:
            return k
    return len(tokens) - 1


def chain_bounds(tokens, depth, dot):
    """The builder chain the method call at `dot` belongs to."""
    level = depth[dot]
    lo = dot
    while lo > 0:
        k = lo - 1
        if depth[k] < level:
            break
        if depth[k] == level and tokens[k][0] == "punct" and tokens[k][1] in CHAIN_STOP:
            break
        if depth[k] == level and tokens[k] == ("ident", "return"):
            break
        lo = k
    hi = dot
    while hi + 1 < len(tokens):
        k = hi + 1
        if depth[k] < level:
            break
        if depth[k] == level and tokens[k][0] == "punct" and tokens[k][1] in CHAIN_STOP:
            break
        hi = k
    return lo, hi


def suggestions_in(tokens, depth, lo, hi, consts):
    found = []
    for k in range(lo, hi):
        if tokens[k] == ("ident", "with_suggestion") and tokens[k + 1] == ("punct", "("):
            end = matching_close(tokens, depth, k + 1)
            parts = []
            for kind, value in tokens[k + 2 : end]:
                if kind == "str":
                    parts.append(value)
                elif kind == "ident" and value in consts:
                    parts.append(consts[value])
            if parts:
                found.append(" ".join(parts))
    return found


def delivered_chains(tokens, depth):
    """Yields (lo, hi) for every chain that stamps a delivered or uncertain disposition."""
    for k in range(len(tokens) - 4):
        if (
            tokens[k] == ("ident", "with_disposition")
            and k > 0
            and tokens[k - 1] == ("punct", ".")
            and tokens[k + 1] == ("punct", "(")
        ):
            end = matching_close(tokens, depth, k + 1)
            inner = tokens[k + 2 : end]
            names = [v for kind, v in inner if kind == "ident"]
            if "DeliverySemantics" in names and any(DISPOSITION.match(v) for v in names):
                if not any(v in ("if", "match") for v in names):
                    yield chain_bounds(tokens, depth, k - 1)
        if (
            tokens[k] == ("ident", "DeliveryTracker")
            and tokens[k + 1] == ("punct", "::")
            and tokens[k + 2] == ("ident", "uncertain")
            and tokens[k + 3] == ("punct", "(")
        ):
            yield k + 3, matching_close(tokens, depth, k + 3)


def prepared(text: str):
    tokens = drop_test_items(tokenize(text))
    return tokens, depths(tokens), string_consts(tokens)


def call_args(tokens, depth, open_at):
    """The argument token ranges of the call whose `(` is at `open_at`."""
    end = matching_close(tokens, depth, open_at)
    level = depth[open_at] + 1
    args, start = [], open_at + 1
    for k in range(open_at + 1, end):
        if depth[k] == level and tokens[k] == ("punct", ","):
            args.append((start, k))
            start = k + 1
    if start < end:
        args.append((start, end))
    return args


def suggestion_helpers(tokens, depth) -> dict[str, int]:
    """Functions that stamp a delivered or uncertain disposition on a suggestion parameter."""
    helpers = {}
    for k in range(len(tokens) - 3):
        if tokens[k] != ("ident", "fn") or tokens[k + 1][0] != "ident":
            continue
        name, open_at = tokens[k + 1][1], k + 2
        if tokens[open_at] != ("punct", "("):
            continue
        params = []
        for lo, hi in call_args(tokens, depth, open_at):
            idents = [v for kind, v in tokens[lo:hi] if kind == "ident" and v != "mut"]
            params.append(idents[0] if idents else "")
        body = open_at
        while body < len(tokens) and tokens[body] != ("punct", "{"):
            if tokens[body] == ("punct", ";"):
                break
            body += 1
        if body >= len(tokens) or tokens[body] != ("punct", "{"):
            continue
        body_end = matching_close(tokens, depth, body)
        for lo, hi in delivered_chains(tokens, depth):
            if not body < lo < body_end:
                continue
            for j in range(lo, hi):
                if (
                    tokens[j] == ("ident", "with_suggestion")
                    and tokens[j + 1] == ("punct", "(")
                    and tokens[j + 2][0] == "ident"
                    and tokens[j + 3] == ("punct", ")")
                    and tokens[j + 2][1] in params
                ):
                    helpers[name] = params.index(tokens[j + 2][1])
    return helpers


def helper_suggestions(tokens, depth, consts, helpers):
    for k in range(len(tokens) - 1):
        if tokens[k][0] != "ident" or tokens[k][1] not in helpers or tokens[k + 1] != ("punct", "("):
            continue
        if k > 0 and tokens[k - 1] == ("ident", "fn"):
            continue
        args = call_args(tokens, depth, k + 1)
        position = helpers[tokens[k][1]]
        if position < len(args):
            lo, hi = args[position]
            parts = [
                v if kind == "str" else consts[v]
                for kind, v in tokens[lo:hi]
                if kind == "str" or (kind == "ident" and v in consts)
            ]
            if parts:
                yield " ".join(parts)


def needs_look_first(suggestion: str) -> bool:
    spoken = OBSERVED_AGAIN.sub("", suggestion)
    return bool(RETRY_WORDS.search(spoken)) and not LOOK_FIRST.search(spoken)


def violations(text: str, helpers: dict[str, int] | None = None) -> list[str]:
    tokens, depth, consts = prepared(text)
    helpers = suggestion_helpers(tokens, depth) if helpers is None else helpers
    found = []
    for lo, hi in delivered_chains(tokens, depth):
        found.extend(suggestions_in(tokens, depth, lo, hi + 1, consts))
    found.extend(helper_suggestions(tokens, depth, consts, helpers))
    return [suggestion for suggestion in found if needs_look_first(suggestion)]


def production_files(root: Path):
    for base in ("crates", "src"):
        for path in sorted((root / base).rglob("*.rs")):
            name = path.name
            parts = path.relative_to(root).parts
            if name.endswith("_tests.rs") or name == "tests.rs" or "test_support" in name:
                continue
            if "tests" in parts[:-1] or "target" in parts:
                continue
            yield path


RUNTIME_DELIVERED_KINDS = {
    "chain_deadline",
    "chain_step_limit",
    "popup_not_observed",
    "scroll_into_view_unsupported",
    "shell_container_unproven",
}
CODE_SPAN = re.compile(r"`[^`]*`")
KIND_SPAN = re.compile(r"`(?:details\.kind: )?\"?([a-z][a-z0-9_]+)\"?`")


def delivered_kinds(text: str) -> set[str]:
    """The `details.kind` names that a delivered or uncertain chain stamps."""
    tokens, depth, _ = prepared(text)
    kinds = set()
    for lo, hi in delivered_chains(tokens, depth):
        for k in range(lo, min(hi, len(tokens) - 2)):
            if (
                tokens[k] == ("str", "kind")
                and tokens[k + 1] == ("punct", ":")
                and tokens[k + 2][0] == "str"
            ):
                kinds.add(tokens[k + 2][1])
    return kinds


def skill_passages(text: str):
    """Table rows, list items and paragraphs of a skill page."""
    passage: list[str] = []
    for line in text.replace("\r\n", "\n").split("\n"):
        stripped = line.strip()
        starts_item = stripped.startswith(("|", "- ", "* ")) or stripped.startswith("#")
        if not stripped or starts_item:
            if passage:
                yield " ".join(passage)
            passage = []
        if stripped.startswith("|"):
            yield stripped
        elif stripped:
            passage.append(stripped)
    if passage:
        yield " ".join(passage)


def skill_violations(text: str, kinds: set[str]) -> list[str]:
    """Passages that name a may-be-delivered error kind and advise a blind retry."""
    found = []
    for passage in skill_passages(text):
        named = {match.group(1) for match in KIND_SPAN.finditer(passage)}
        if named & kinds and needs_look_first(CODE_SPAN.sub("", passage)):
            found.append(passage)
    return found


def skill_files(root: Path):
    yield from sorted((root / "skills").rglob("*.md"))


SELF_TEST_BAD = [
    """fn a() -> AdapterError {
        AdapterError::new(ErrorCode::ActionFailed, "x")
            .with_suggestion("Retry the dismissal")
            .with_disposition(DeliverySemantics::delivered_unverified())
    }""",
    """const HINT: &str = "Run it again";
    fn b() -> AdapterError {
        Err(AdapterError::timeout("x").with_disposition(DeliverySemantics::uncertain()).with_suggestion(HINT))
    }""",
    """fn c() -> AdapterError {
        DeliveryTracker::uncertain(AdapterError::new(code, "x").with_suggestion(format!("Rerun {x}")))
    }""",
    """fn pressed(message: &str, suggestion: &str) -> AdapterError {
        AdapterError::new(code, message)
            .with_suggestion(suggestion)
            .with_disposition(DeliverySemantics::delivered_unverified())
    }
    fn e() -> AdapterError {
        pressed("The entry was pressed again", "Run list-notifications and retry")
    }""",
    """fn d() -> AdapterError {
        AdapterError::new(code, "x")
            .with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified())
            .with_suggestion(
                "Close the overview, \\
                 then retry",
            )
    }""",
]
SELF_TEST_GOOD = [
    """fn a() -> AdapterError {
        AdapterError::new(code, "x")
            .with_suggestion("Retry the command")
            .with_disposition(DeliverySemantics::not_delivered())
    }""",
    """fn b() -> AdapterError {
        AdapterError::new(code, "x")
            .with_suggestion("Read the state before deciding whether to retry")
            .with_disposition(DeliverySemantics::uncertain())
    }""",
    """fn c() -> AdapterError {
        let first = AdapterError::new(code, "x").with_suggestion("Retry");
        first.with_disposition(DeliverySemantics::delivered_unverified())
    }""",
    """fn pressed(message: &str, suggestion: &str) -> AdapterError {
        AdapterError::new(code, message)
            .with_suggestion(suggestion)
            .with_disposition(DeliverySemantics::delivered_unverified())
    }
    fn e() -> AdapterError {
        pressed("The entry was pressed again", "Run list-notifications to see what it holds now")
    }""",
    """fn f() -> AdapterError {
        AdapterError::new(code, "x")
            .with_suggestion("Run 'list-windows' to find the window again")
            .with_disposition(DeliverySemantics::delivered_unverified())
    }""",
    """#[cfg(test)]
    mod tests {
        fn d() { AdapterError::new(code, "x").with_suggestion("Retry").with_disposition(DeliverySemantics::uncertain()); }
    }""",
]


def self_test() -> int:
    failed = 0
    for case in SELF_TEST_BAD:
        if not violations(case):
            print(f"self-test: missed a planted violation:\n{case}", file=sys.stderr)
            failed += 1
    for case in SELF_TEST_GOOD:
        if violations(case):
            print(f"self-test: flagged a compliant case:\n{case}", file=sys.stderr)
            failed += 1
    planted = {"popup_not_observed"}
    if delivered_kinds(SELF_TEST_KIND_CHAIN) != planted:
        print("self-test: missed the kind of a delivered chain", file=sys.stderr)
        failed += 1
    for page, expected in SELF_TEST_SKILL_PAGES:
        if bool(skill_violations(page, planted)) != expected:
            print(f"self-test: skill passage misjudged:\n{page}", file=sys.stderr)
            failed += 1
    return failed


SELF_TEST_KIND_CHAIN = """fn a() -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "x")
        .with_details(json!({ "kind": "popup_not_observed" }))
        .with_disposition(DeliverySemantics::delivered_unverified())
}
fn b() -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "y")
        .with_details(json!({ "kind": "safe_refusal" }))
        .with_disposition(DeliverySemantics::not_delivered())
}"""

SELF_TEST_SKILL_PAGES = [
    ("| `ACTION_FAILED` | A click that showed no popup (`popup_not_observed`). Close it and retry |", True),
    ("- A `popup_not_observed` click may have landed. Retry it.", True),
    ("| `ACTION_FAILED` | `popup_not_observed`: take a snapshot before you decide whether to retry |", False),
    ("- `popup_not_observed` carries `retry: unsafe`.", False),
    ("| `ACTION_FAILED` `other_kind` | Close it and retry |", False),
]


def main(argv: list[str]) -> int:
    if self_test():
        print("FAIL: the retry-advice scanner self-test failed", file=sys.stderr)
        return 1
    if argv[1:] == ["--self-test"]:
        print("OK: retry-advice scanner self-test passed")
        return 0
    root = Path(__file__).resolve().parent.parent
    sources = {path: path.read_text(encoding="utf-8") for path in production_files(root)}
    helpers: dict[str, int] = {}
    for text in sources.values():
        tokens, depth, _ = prepared(text)
        helpers.update(suggestion_helpers(tokens, depth))
    found = 0
    kinds = set(RUNTIME_DELIVERED_KINDS)
    for path, text in sources.items():
        kinds |= delivered_kinds(text)
        for suggestion in violations(text, helpers):
            print(f"{path.relative_to(root)}: {suggestion}", file=sys.stderr)
            found += 1
    for path in skill_files(root):
        for passage in skill_violations(path.read_text(encoding="utf-8"), kinds):
            print(f"{path.relative_to(root)}: {passage}", file=sys.stderr)
            found += 1
    if found:
        print(
            f"FAIL: {found} suggestion(s) say to retry on an error that may already be delivered. "
            "Tell the caller to read the state first (for example 'before deciding whether to retry').",
            file=sys.stderr,
        )
        return 1
    print("OK: no retry advice on delivered or uncertain errors, in code or skills (self-test passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
