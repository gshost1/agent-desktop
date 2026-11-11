# Held-out tasks (frozen 2026-11-10, before any run)

18 tasks: 10 web (7 on the bench's local site, 3 on public read-only sites) and 8 native macOS. Each one adapts a task from a public benchmark, cited below with its id and original intent. The full definitions, prompts and verifiers are in `tasks_heldout.py`. The local site is `web/server.py`, and the native verifier helper is `ho_probe.swift`.

## How they were chosen

- They were picked from the benchmark task lists (WebArena `config_files/test.raw.json`, WebVoyager `data/WebVoyager_data.jsonl`, OSWorld `evaluation_examples/`) before any tool ran on them. No task was tried with agent-desktop or any other arm, and none was changed after a run.
- **No overlap with existing bench tasks.** The existing set is weather, settings (About), calculator, w1 (order total), w2 (shipping form), w3 (subscribe wizard), w4 (inventory lookup), r1 (Wikipedia), r2 (GitHub release), r3 (weather.gov), r4 (docs.python.org), r5 (httpbin form), and the native set: settings-dock, settings-appearance, settings-trackpad, settings-battery, calc-chain, about-to-calc. No held-out task uses those pages, panes or apps for the same job. A WebVoyager task about GitHub releases (GitHub--8/12/35) was skipped because r2 does the same. Huggingface--13 was dropped because the parameter it asks about is gone from today's docs.
- Every arm gets the same task prompt. Only the tool paragraph of the system prompt differs (`arms/common.py`, `arms/run_web.py`).
- Changes from the source tasks: the local sites are small re-implementations of the WebArena sites, with seeded data that differs per run and is identical across arms (`web_seed(task, run)`). Relative dates such as "the past month" are given as explicit dates. Ubuntu, LibreOffice and GIMP tasks are moved to macOS TextEdit, Preview and System Settings. The two OSWorld settings tasks that change a setting became read-only versions, because the bench never changes a setting on George's Mac.

## Web

| Id | Source | Original intent | Held-out task | Verifier (independent of the agent's claim) |
|---|---|---|---|---|
| hw1 | WebArena 21 (shopping) | List out reviewers, if exist, who mention about ear cups being small | Product page with 21–23 reviews over 3 pages: 3–5 ear-cup complaints in varied wording, 6 distractors ("ear cups are soft and roomy", "small carrying case") | Answer names = the seeded complainers exactly; any distractor name fails |
| hw2 | WebArena 47 (shopping) | Today is 6/12/2023. How many fulfilled orders over the past month, and the total spent? | Paginated order history of 28 orders (10 per page), with orders on the window start and the day before it | COUNT and TOTAL both equal the values computed from the seed |
| hw3 | WebArena 777 (shopping_admin) | Reduce the price of green Hollister backyard sweater in all size by $5 | Admin product grid with search; 5 sizes × 3 colours of one family, plus 56 other products | Server-side prices: all 5 targets at original − $5.00, no other product changed |
| hw4 | WebArena 404 (reddit) | Upvote the newest post in books subreddit | Forum sorted by hot by default, so the newest books post is never first; other forums hold newer posts | Server vote state is exactly {newest books post: +1} |
| hw5 | WebArena 409 (reddit) | Reply to the manager of the website in this post with "thanks! I am a big fan of your website." | Thread with nested comments; the manager is marked MOD, and another user quotes the manager | Exactly one new comment, parented to the MOD comment, with the exact text |
| hw6 | WebArena 658 (gitlab) | Create an issue in a11yproject repo with title "401 bad gateway", assign to Roshanjossey, due date end of 2030 | Start at the project list; issue form with a 16-member assignee select and a date input; due date "end of <Month YYYY>" | Exactly one issue in that project: title, assignee username and last-day-of-month due date |
| hw7 | WebArena 269 (shopping, url_match) | Show me products under $25 in "women shoes" category | Category listing with $20 price buckets (none gives "under $25") and a min/max form; no price within $0.50 of $25 | The last shop page the browser loaded is a Women Shoes listing whose filter shows exactly the products under $25 |
| hp1 | WebVoyager ArXiv--17 | Find the paper 'GPT-4 Technical Report', when was v3 submitted? | Same, on arxiv.org | The verifier reads the [v3] date from arxiv.org/abs/2303.08774 just before the run; the answer must name that date |
| hp2 | WebVoyager ArXiv--27 | What categories does Economics include, and what are their abbreviations? | Same | The verifier reads econ.* codes and names from arxiv.org/category_taxonomy; the answer must contain every code and name |
| hp3 | WebVoyager Cambridge Dictionary--17 | How many meanings of "unblemished" are given in Cambridge Dictionary? | Same | The verifier counts the distinct definitions on the word's page just before the run (2 on 2026-11-10) |

Public tasks carry the bench's rule: no login, no submit, decline cookie banners. A run whose expected value cannot be fetched is invalid and is not counted as a failure.

## Native (macOS)

| Id | Source | Original intent | Held-out task | Verifier |
|---|---|---|---|---|
| hn1 | OSWorld 0b17a146-2934-46c7-8727-73ff6b6483e8 (libreoffice_writer) | Change the 2 in "H2O" to a subscript | TextEdit, a bench RTF with two "H2O" and one "CO2" as a distractor; save in place | `ho_probe rtf`: the text is unchanged, both H2O 2s are subscript, and nothing else is |
| hn2 | OSWorld 72b810ef-4156-4d09-8f08-a0cf57e7cefe (libreoffice_writer) | Add strike-through on words in the last paragraph | TextEdit, a 5-paragraph course outline | The last paragraph is fully struck through, the others not at all; text unchanged |
| hn3 | OSWorld 3ef2b351-8a84-4ff2-8724-d86eae9b842e (libreoffice_writer) | Center align the heading | TextEdit, a heading and 3 paragraphs | The heading is centred, no other paragraph is; text unchanged |
| hn4 | OSWorld e528b65e-1107-4b8c-8988-490e4fece599 (libreoffice_writer) | Make the first letter of each word uppercase | TextEdit, 3 lowercase lines | The text equals the word-capitalised original |
| hn5 | OSWorld 72f83cdc-bf76-4531-9a1b-eb893a13f8aa (gimp) | Mirror the figure horizontally | Preview, a bench 600×400 PNG with asymmetric colour blocks; save in place, as PNG | Still PNG, still 600×400, and a 12×8 colour-class grid matches the mirrored seed (≤ 2 cells off) |
| hn6 | OSWorld d16c99dc-2a1e-46f2-b350-d97c86c85c15 (gimp) | Resize to height 512 keeping the aspect ratio | Preview, the same PNG | Still PNG, 768±1 × 512, and the grid matches the scaled seed |
| hn7 | OSWorld b6781586-6346-41cd-935a-a6b1487918fc (os) | Set the time zone to UTC+0 | Read-only: report the time zone shown in General > Date & Time | `/etc/localtime` is unchanged; the answer names the zone (IANA id, city, or a name macOS gives it) |
| hn8 | OSWorld bedcedc4-4d72-425e-ad62-21960b11fe0d (os) | Set 'Dim screen when inactive' to off | Read-only: report when the display turns off on power adapter (Lock Screen) | `pmset` AC displaysleep is unchanged, and the answer's minutes (or Never) equal it |

**Privacy.** The native tasks open only files the task creates in `~/agent-desktop-bench/bench-ho-<task>-<run>/`, named `seed-ho-*`, by full path. The app is started with window restoration off, and the run stops before the model starts if the app shows any non-bench window. No task opens a file panel: every save is in place. System Settings is only read, and the run fails if a value changed. The wrappers refuse whole-screen reads.

**Caps.** Native runs are capped at 300 s, web runs at 600 s.

## Frozen hashes

`run_all.sh` and `preflight.sh` refuse to run when `tasks_heldout.py` differs from this line.

- tasks_heldout.py sha256 `55a13cb15cec84e348630caf2d89dc254a6b6beaafc5ea0405b6ae56ecdac7c8`
- web/server.py sha256 `b6b60b534ed17aea189fc004973436514128874c3b831551b0f28507f2be8f0f`
- ho_probe.swift sha256 `b9ce7173673ba88a85e20d4bec09aab0676c5e607b8eb40f45808e020bf4379a`
