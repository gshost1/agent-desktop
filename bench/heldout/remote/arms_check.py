"""Refuse an arms pass unless the task set and every candidate arm match their pins in the set's FROZEN.md.

python3 -I arms_check.py <taskset module> <candidates dir> <arms dir> <arm> [<arm> ...]

Exits 0 when every file pin of the set matches and the module itself is pinned, every arm is known, no arm is
listed twice, and every candidate arm's agent-desktop and agent-desktop-macos-helper match an
"arm `<label>` <file> sha256 `<hex>`" line. A set whose FROZEN.md names "manifest digest `<hex>`" must also
rebuild to that digest on this host (remote/manifest.py over these arms), and a set that pins `schedule.json`
runs only with SCHEDULE set to that file. A placeholder pin (`<at freeze>`) refuses. Prints each refusal
otherwise."""
import hashlib
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

FILE_PIN = re.compile(r"`([^`]+)` sha256 `([0-9a-f]{64})`")
ARM_PIN = re.compile(r"arm `([^`]+)` (agent-desktop(?:-macos-helper)?) sha256 `([0-9a-f]{64})`")
MANIFEST_PIN = re.compile(r"manifest digest `([0-9a-f]{64})`")
CANDIDATE_FILES = ("agent-desktop", "agent-desktop-macos-helper")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def refusals(taskset, candidates, builtin, arms):
    root = taskset.parent
    frozen = root / "FROZEN.md"
    if not frozen.is_file():
        return [f"no FROZEN.md beside {taskset.name}"]
    text = frozen.read_text()
    files = FILE_PIN.findall(text)
    out = [f"{f} differs from its pin" for f, h in files if not (root / f).is_file() or sha(root / f) != h]
    if taskset.name not in {f for f, _ in files}:
        out.append(f"{taskset.name} is not pinned")
    pins = {(label, f): h for label, f, h in ARM_PIN.findall(text)}
    for arm in arms:
        cand = candidates / arm
        if (cand / "agent-desktop").is_file():
            for f in CANDIDATE_FILES:
                want = pins.get((arm, f))
                if want is None:
                    out.append(f"candidate {arm}: {f} is not pinned")
                elif not (cand / f).is_file() or sha(cand / f) != want:
                    out.append(f"candidate {arm}: {f} differs from its pin")
        elif arm not in builtin:
            out.append(f"unknown arm {arm}")
    if len(set(arms)) != len(arms):
        out.append("an arm is listed twice")
    if "<at freeze>" in text:
        out.append("FROZEN.md still has a placeholder pin")
    if "schedule.json" in {f for f, _ in files}:
        given = os.environ.get("SCHEDULE")
        if not given or Path(given).resolve() != (root / "schedule.json").resolve():
            out.append("the set pins schedule.json; run with SCHEDULE set to it")
    want = MANIFEST_PIN.search(text)
    if want and not out:
        import manifest
        got = manifest.digest(manifest.build(taskset, arms))
        if got != want.group(1):
            out.append(f"manifest digest {got[:12]} differs from its pin {want.group(1)[:12]}")
    return out


def main():
    taskset, candidates, arms_dir = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    sys.path.insert(0, str(arms_dir))
    import common
    bad = refusals(taskset, candidates, set(common.NATIVE_TOOL), sys.argv[4:])
    for line in bad:
        print(line)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
