#!/usr/bin/env python3
"""Rewrite saved snapshot envelopes into candidate output formats.

Offline: no app is read. Each variant keeps every ref and every name/value the
agent needs; only presentation changes. Token counts come from
bench/token_calibrate.py run over the written files.

Variants:
  v0  as shipped
  v1  drop available_actions entries that never qualify a ref (RightClick, ScrollTo, SetFocus)
  v2  v1 + native_id flattened to "id": "<value>"
  v3  v2 + refs as "e12" with the snapshot id once at the top
  v4  v3 + collapse unnamed, ref-less single-child wrappers (what --compact intends)
  v5  v4 as an indented text outline: `role "name" [e12] value=... actions` one node per line

Usage: python3 bench/format_variants.py SNAPSHOT.json... -> bench/out/variants/<name>.<variant>.<ext>
"""
import copy
import json
import sys
from pathlib import Path

OUT = Path(__file__).resolve().parent / "out" / "variants"
NON_QUALIFYING = {"RightClick", "ScrollTo", "SetFocus"}


def strip_actions(node):
    acts = [a for a in node.get("available_actions", []) if a not in NON_QUALIFYING]
    if acts:
        node["available_actions"] = acts
    else:
        node.pop("available_actions", None)
    for c in node.get("children", []):
        strip_actions(c)


def flatten_ids(node):
    nid = node.pop("native_id", None)
    if isinstance(nid, dict) and nid.get("value"):
        node["id"] = nid["value"]
    for c in node.get("children", []):
        flatten_ids(c)


def short_refs(node):
    if "ref_id" in node:
        node["ref"] = node.pop("ref_id").split(":")[-1]
    for c in node.get("children", []):
        short_refs(c)


def is_wrapper(node):
    keys = set(node) - {"role", "children"}
    return node.get("role") == "group" and not keys and len(node.get("children", [])) == 1


def collapse(node):
    kids = []
    for c in node.get("children", []):
        c = collapse(c)
        while is_wrapper(c):
            c = c["children"][0]
        kids.append(c)
    if kids:
        node["children"] = kids
    return node


def outline(node, depth=0, lines=None):
    lines = [] if lines is None else lines
    parts = [node.get("role", "?")]
    if node.get("name"):
        parts.append(json.dumps(node["name"], ensure_ascii=False))
    if node.get("ref"):
        parts.append(f"[{node['ref']}]")
    if node.get("value") not in (None, ""):
        parts.append("value=" + json.dumps(node["value"], ensure_ascii=False))
    if node.get("description"):
        parts.append("desc=" + json.dumps(node["description"], ensure_ascii=False))
    if node.get("id"):
        parts.append(f"#{node['id']}")
    if node.get("states"):
        parts.append("{" + ",".join(node["states"]) + "}")
    if node.get("available_actions"):
        acts = [a for a in node["available_actions"] if a != "Click"]
        if acts:
            parts.append("(" + ",".join(acts) + ")")
    if node.get("children_count"):
        parts.append(f"+{node['children_count']}")
    lines.append("  " * depth + " ".join(parts))
    for c in node.get("children", []):
        outline(c, depth + 1, lines)
    return lines


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for f in sys.argv[1:]:
        env = json.loads(Path(f).read_text())
        stem = Path(f).stem
        data = env["data"]
        variants = {"v0": copy.deepcopy(env)}
        e = copy.deepcopy(env)
        strip_actions(e["data"]["tree"])
        variants["v1"] = copy.deepcopy(e)
        flatten_ids(e["data"]["tree"])
        variants["v2"] = copy.deepcopy(e)
        short_refs(e["data"]["tree"])
        variants["v3"] = copy.deepcopy(e)
        e["data"]["tree"] = collapse(e["data"]["tree"])
        variants["v4"] = copy.deepcopy(e)
        for k, v in variants.items():
            (OUT / f"{stem}.{k}.json").write_text(json.dumps(v, ensure_ascii=False, separators=(",", ":")))
        head = f"snapshot {data['snapshot_id']} app={data.get('app')} refs={data.get('ref_count')} complete={data.get('complete')}"
        text = head + "\n" + "\n".join(outline(e["data"]["tree"])) + "\n"
        (OUT / f"{stem}.v5.txt").write_text(text)
        print(stem, {k: len(json.dumps(v, ensure_ascii=False, separators=(',', ':')).encode()) for k, v in variants.items()}, "v5", len(text.encode()))


if __name__ == "__main__":
    main()
