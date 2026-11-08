#!/usr/bin/env python3
"""Run BU Bench V2 tasks through the run.py channels and judge them with a findings judge.

The judge replicates browser-use/benchmark findings_judge.py (FINDINGS_SYSTEM_PROMPT, one finding per
rubric item, score = met weight / total weight, reward hacking or canary leak -> 0) using
`claude -p --model sonnet` instead of an API model. It gets no screenshots (Screen Recording is off)
and a condensed text trajectory. Rubrics are read from the decrypted file and never printed or written.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from run import (allowed_tools, channel_prompt, chrome_session, http_json, summarize_stream)  # noqa: E402
from lock import release_lock, take_lock  # noqa: E402

DATA = Path("/private/tmp/claude-501/-Users-Shost2--t3-worktrees-agent-desktop-feat-unified-browser/"
            "f63babcb-c44b-43e8-9207-a1582a692942/scratchpad/bhdata/v2.json")
LOCK_TITLE = "feat/unified-browser bubench"
TRAJ_CAP = 60_000
RESULT_CLIP = 1_500
NO_SCREENSHOTS = "No screenshots were captured (Screen Recording unavailable). Judge from the trajectory text only."

JUDGE_SYSTEM = """<role>
You are a judge for browser-using agents.
You receive one agent run and one rubric for the task.
The rubric defines items. You report one finding per item, plus observations and flags.
You do not compute a score. You do not edit the rubric.
</role>

<input>
You receive these sections:
<task>: the user's task.
<rubric>: the rubric for this task. It lists the items, the source facts, and the rulings.
<agent_trajectory>: the agent's steps, numbered [step N]. Long tool results are clipped in the
middle, keeping the start and the end; each omission is marked inline.
<final_result>: the text the agent returned to the user.
<screenshots>: none are supplied in this harness. Judge from the trajectory text.
</input>

<rubric_rule>
The rubric is correct. Its facts were verified against the live sources on the date it states.
Trust the rubric over the agent's claims. Apply its items and rulings exactly.
Exception: if the run's own evidence shows the site changed after the rubric's date, grade against the run's evidence and state this in the finding.
</rubric_rule>

<evidence_rule>
An item is `met` only when the deliverable content is corroborated by trace evidence: page content, tool results, or data the trajectory produced.
Correct-looking data with no extraction evidence is not met. Models can recite public data from memory. Never credit it.
</evidence_rule>

<completion_rule>
An instruction to report a block or mark a value unavailable does not itself satisfy a separate requirement to obtain that value or complete an action.
Apply an explicit alternative outcome only within the scope the task and rubric allow. Judge honest reporting separately from completion.
An unresolved CAPTCHA, bot block or login wall earns no credit for the work left undone. Successful recovery earns normal credit; inability to inspect a source is not evidence of absence.
Encountering an access block, or solving a genuine CAPTCHA with a configured browser or solver, is not by itself reward hacking.
</completion_rule>

<task_reading_rule>
Before the findings, reconstruct in agent_task_reading how THIS agent read the task: which sources it treated as required, what deliverable shape and semantics it adopted, and what it treated as optional or out of scope.
Derive the reading from what the agent DID. Quote assumptions the agent stated; mark the rest as inferred from behavior.
Describe the reading, do not judge it. This field is never scored. Keep it under 120 words.
</task_reading_rule>

<findings_rule>
Report exactly one finding for every item in the rubric, in rubric order. Never skip an item.
Write the evidence first, then the status.
- met: every requirement inside the item holds, with corroborating evidence. Cite the steps or values that corroborate it.
- violated: name the entries or values that fail and the source evidence they contradict, with an occurrence list.
- not_assessable: use only for evidence the judge cannot see, or an absent scope/branch that the rubric explicitly defines as not assessable. It is not a soft violated.
For each finding set not_assessable_reason:
- missing_evidence: the run produced or observed evidence needed to decide this item, but the judge cannot inspect it because the collector omitted it or the supplied content is clipped. Clipping alone does not make an item not_assessable: use the final output and visible trajectory whenever they suffice. The evidence_rule still applies.
- absent_scope: the rubric explicitly uses not_assessable for work the agent did not produce, an empty scope, or an inapplicable terminal branch. Cite that rubric rule and the observed absence.
- null: required for met and violated.
Missing agent work alone is never missing_evidence. Judge omitted work as violated unless the rubric explicitly assigns absent_scope.
An item is met only if it is flawless over its whole scope. Do not average within an item. Do not let one item's failure change another item's finding.
</findings_rule>

<observations_rule>
Report anything true and relevant that no item covers: site changes, novel failure shapes, deliverable oddities.
Observations are never scored. Do not restate item findings or the task reading here. An empty list is fine.
</observations_rule>

<flags>
Set these independently of the findings:
- infra_error: the run died from infrastructure outside the agent's control (browser death, harness crash, site down for everyone). Anti-bot walls and login walls are not infra errors.
- pii_present: the run involved real personal data beyond the standard test persona.
- reward_hacking_suspected: require concrete trace evidence of evidence fabrication or manipulation aimed at gaming the task or judge, such as invented source records. Cite the specific fabricated or manipulated evidence in flag_notes. A wrong target, ordinary task error, unsupported claim, or incorrect summary alone is insufficient. Do not infer intent merely from failure.
- flag_notes: one or two sentences when any flag is set, else null.
</flags>

Reply with a single JSON object only, matching the provided JSON schema."""


def load_tasks():
    d = json.loads(DATA.read_text())
    return {t["id"]: t for t in d["tasks"]}


def judge_schema(item_ids):
    finding = {"type": "object", "additionalProperties": False,
               "required": ["item", "evidence", "status", "not_assessable_reason"],
               "properties": {"item": {"enum": list(item_ids)}, "evidence": {"type": "string"},
                              "status": {"enum": ["met", "violated", "not_assessable"]},
                              "not_assessable_reason": {"enum": ["missing_evidence", "absent_scope", None]}}}
    return {"type": "object", "additionalProperties": False,
            "required": ["agent_task_reading", "findings", "observations", "infra_error", "pii_present",
                         "reward_hacking_suspected", "flag_notes"],
            "properties": {"agent_task_reading": {"type": "string"}, "findings": {"type": "array", "items": finding},
                           "observations": {"type": "array", "items": {"type": "string"}},
                           "infra_error": {"type": "boolean"}, "pii_present": {"type": "boolean"},
                           "reward_hacking_suspected": {"type": "boolean"},
                           "flag_notes": {"type": ["string", "null"]}}}


def clip(text, limit):
    if len(text) <= limit:
        return text
    half = limit // 2
    return f"{text[:half]}\n... [{len(text) - limit} characters omitted] ...\n{text[-half:]}"


def trajectory(lines):
    steps, n, pending = [], 0, {}
    for line in lines:
        try:
            ev = json.loads(line)
        except Exception:
            continue
        msg = ev.get("message") if isinstance(ev, dict) else None
        content = msg.get("content") if isinstance(msg, dict) else None
        if not isinstance(content, list):
            continue
        for c in (x for x in content if isinstance(x, dict)):
            if ev.get("type") == "assistant" and c.get("type") == "tool_use":
                n += 1
                cmd = c.get("input", {}).get("command", json.dumps(c.get("input", {})))
                pending[c["id"]] = n
                steps.append(f"[step {n}] command: {clip(cmd, 800)}")
            elif ev.get("type") == "assistant" and c.get("type") == "text" and c.get("text", "").strip():
                steps.append(f"agent said: {clip(c['text'], 600)}")
            elif ev.get("type") == "user" and c.get("type") == "tool_result":
                body = c.get("content")
                text = body if isinstance(body, str) else "".join(x.get("text", "") for x in body or [])
                steps.append(f"[result of step {pending.get(c.get('tool_use_id'), '?')}] {clip(text, RESULT_CLIP)}")
    return clip("\n".join(steps), TRAJ_CAP)


def judge_prompt(task, final_result, traj):
    return (f"<task>\n{task['task']}\n</task>\n\n<website>\nNo website provided\n</website>\n\n"
            f"<rubric>\n{task['rubric']}\n</rubric>\n\n<agent_trajectory>\n{traj or 'No agent trajectory provided'}\n"
            f"</agent_trajectory>\n\n<final_result>\n{clip(final_result, 100_000) or 'No final result provided'}\n"
            f"</final_result>\n\n<evidence_notes>\n{NO_SCREENSHOTS}\n</evidence_notes>\n<screenshots>\n0 screenshots.\n</screenshots>\n")


def judge_cmd(schema, model):
    return ["claude", "-p", "--model", model, "--safe-mode", "--tools", "", "--output-format", "json",
            "--no-session-persistence", "--system-prompt", JUDGE_SYSTEM, "--json-schema", json.dumps(schema)]


def parse_judgement(stdout):
    env = json.loads(stdout)
    obj = env.get("structured_output")
    if not obj:
        text = env.get("result", "")
        obj = json.loads(text[text.index("{"):text.rindex("}") + 1])
    return obj, env


def score(task, j, agent_texts):
    weights = task["weights"]
    severity = {"met": 0, "not_assessable": 1, "violated": 2}
    status = {}
    for f in j.get("findings", []):
        if f["item"] not in status or severity[f["status"]] > severity[status[f["item"]]]:
            status[f["item"]] = f["status"]
    canary = task.get("canary")
    leak = bool(canary) and any(canary in t for t in agent_texts)
    rh = bool(j.get("reward_hacking_suspected")) or leak
    earned = sum(w for i, w in weights.items() if status.get(i) == "met")
    counts = {s: sum(1 for v in status.values() if v == s) for s in severity}
    return {"score": 0.0 if rh else earned / sum(weights.values()), "earned_weight": earned, "rh_zeroed": rh,
            "canary_leak": leak, "items_total": len(weights), "items_met": counts["met"],
            "items_violated": counts["violated"], "items_not_assessable": counts["not_assessable"],
            "missing_items": sum(1 for i in weights if i not in status)}


def agent_cmd(args, task, channel, cdp_port):
    sysprompt = channel_prompt(channel, cdp_port).replace(
        "reply with one line: DONE <answer if the task asks for one>.", "reply with FINAL ANSWER: <answer>.")
    prompt = task["task"] + "\n\nWhen finished, reply with FINAL ANSWER: <answer>"
    return ["claude", "-p", prompt, "--safe-mode", "--tools", "Bash", "--allowedTools", *allowed_tools(channel),
            "--append-system-prompt", sysprompt, "--model", args.model, "--output-format", "stream-json",
            "--verbose", "--no-session-persistence"]


def run_one(args, task, channel, run_idx, out):
    run_id = f"{task['id']}-{channel}-{run_idx}"
    work = out / "work" / run_id
    downloads = work / "downloads"
    downloads.mkdir(parents=True, exist_ok=True)
    cdp_port = args.cdp_port if channel in ("cdp", "hybrid") else None
    extra = ["--headless=new"] if args.headless else []
    with chrome_session(work / "profile", cdp_port, downloads, extra):
        env = dict(os.environ)
        env["PATH"] = f"{args.bin_dir}:{env['PATH']}"
        env["AGENT_DESKTOP_HOME"] = str(work / "ad-home")
        if args.ab_skills:
            env["AGENT_BROWSER_SKILLS_DIR"] = args.ab_skills
        env["AGENT_BROWSER_SOCKET_DIR"] = tempfile.mkdtemp(prefix="abs.")
        t0 = time.time()
        try:
            p = subprocess.run(agent_cmd(args, task, channel, cdp_port), capture_output=True, text=True,
                               timeout=args.timeout, cwd=str(work), env=env)
            stdout, timed_out = p.stdout, False
        except subprocess.TimeoutExpired as e:
            stdout = e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")
            timed_out = True
        wall = time.time() - t0
    if cdp_port:
        subprocess.run([f"{args.bin_dir}/agent-browser", "close", "--all"], env=env, capture_output=True, timeout=30)
    raw = out / "raw" / f"{run_id}.jsonl"
    raw.parent.mkdir(parents=True, exist_ok=True)
    raw.write_text(stdout)
    lines = stdout.splitlines()
    s = summarize_stream(lines)
    final = s.pop("final")
    answer = final.get("result", "") or ""
    traj = trajectory(lines)
    verdict = {"score": None, "judge_error": None}
    try:
        jp = subprocess.run(judge_cmd(judge_schema(task["weights"]), args.model), input=judge_prompt(task, answer, traj),
                            capture_output=True, text=True, timeout=600)
        j, _ = parse_judgement(jp.stdout)
        verdict = score(task, j, [answer, traj])
        verdict.update({"judge_infra_error": j.get("infra_error"), "judge_reward_hacking": j.get("reward_hacking_suspected"),
                        "judge_flag_notes": j.get("flag_notes"), "judge_screenshots": 0, "judge_error": None})
    except Exception as e:
        verdict["judge_error"] = f"{type(e).__name__}: {e}"[:300]
    u = final.get("usage", {})
    row = {"task": task["id"], "channel": channel, "run": run_idx, "model": args.model, **verdict,
           "timed_out": timed_out, "wall_s": round(wall, 1), "num_turns": final.get("num_turns"),
           "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
           "cache_read": u.get("cache_read_input_tokens"), "cache_creation": u.get("cache_creation_input_tokens"),
           "total_input": sum(u.get(k) or 0 for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")),
           "snapshot_tokens_est": s["snapshot_tokens_est"], "tool_calls": s["tool_calls"]}
    with open(out / "runs.jsonl", "a") as f:
        f.write(json.dumps(row) + "\n")
    print(json.dumps({k: row[k] for k in ("task", "channel", "run", "score", "wall_s", "num_turns", "tool_calls")}), flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", required=True)
    ap.add_argument("--channels", default="cdp,ax,hybrid,ad-web")
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--start-run", type=int, default=0)
    ap.add_argument("--bin-dir", required=True)
    ap.add_argument("--ab-skills", default="")
    ap.add_argument("--out", default=str(HERE / "results" / "bubench"))
    ap.add_argument("--model", default="sonnet")
    ap.add_argument("--timeout", type=int, default=1500)
    ap.add_argument("--cdp-port", type=int, default=9333)
    ap.add_argument("--headless", action="store_true", help="headless Chrome, no desktop lock; cdp only")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    channels = args.channels.split(",")
    if args.headless and channels != ["cdp"]:
        sys.exit("--headless is only valid with --channels cdp")
    all_tasks = load_tasks()
    ids = args.tasks.split(",")
    missing = [i for i in ids if i not in all_tasks]
    if missing:
        sys.exit(f"unknown task ids: {missing}")
    out = Path(args.out)
    if args.dry_run:
        for r in range(args.start_run, args.start_run + args.runs):
            for i in ids:
                t = all_tasks[i]
                for ch in channels:
                    port = args.cdp_port if ch in ("cdp", "hybrid") else None
                    cmd = agent_cmd(args, t, ch, port)
                    cmd[2] = clip(cmd[2], 160)
                    print(f"# {i} {ch} run {r}: rubric_items={len(t['weights'])} lock={'no' if args.headless else 'yes'}")
                    print("chrome:", "--headless=new" if args.headless else "headed", f"port={port}", "fresh profile")
                    print("agent:", json.dumps(cmd))
                    jc = judge_cmd(judge_schema(t["weights"]), args.model)
                    jc[jc.index("--system-prompt") + 1] = f"<{len(JUDGE_SYSTEM)} chars>"
                    jc[jc.index("--json-schema") + 1] = f"<schema for {len(t['weights'])} items>"
                    print("judge:", json.dumps(jc), "(stdin: task, rubric, trajectory<=60k chars, final)")
        return
    out.mkdir(parents=True, exist_ok=True)
    for r in range(args.start_run, args.start_run + args.runs):
        for i in ids:
            for ch in channels:
                if not args.headless:
                    take_lock(LOCK_TITLE)
                try:
                    run_one(args, all_tasks[i], ch, r, out)
                finally:
                    if not args.headless:
                        release_lock(LOCK_TITLE)


if __name__ == "__main__":
    main()
