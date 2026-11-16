"""The executable experiment, hashed: everything a final run depends on, built on the host that runs it.

python3 -I manifest.py <taskset module> <arm> [<arm> ...] > manifest.json
python3 -I manifest.py --digest <taskset module> <arm> [...]     # only the digest the pin names

Entries, each a sha256 or a measured value, keyed by a path with the home folder written as ~ so both Macs name
the same file the same way:
  - harness: every .py, .sh, .zsh and arm wrapper under bench/heldout (not runs/ or tests/), bench/driver.py,
    pricing.py, tasks.py, lockq.sh, macctl.swift and the built macctl;
  - task set: every file of its folder that FROZEN.md pins, and schedule.json when present;
  - tools: the verifier binary, and for each arm its wrapper, the binary the wrapper runs and that binary's
    helper beside it, and every file of the arm's guide folder;
  - prompts: the sha256 of each arm's system prompt as the harness builds it;
  - settings: the claude CLI version, HO_RULES, the cap, the macOS version and build, and each task app's
    version and build.
The digest is the sha256 of the sorted JSON. FROZEN.md pins it as "manifest digest `<hex>`"; arms_check rebuilds
it on the host and refuses the pass when it differs, and every row records it.
"""
import hashlib
import json
import os
import plistlib
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
HELDOUT = HERE.parent
BENCH = HELDOUT.parent
HOME = str(Path.home())
EXEC_PATH = re.compile(r'(?:"\$T|\$T|"\$HOME|\$HOME)(/[^"\s]+)')
APP_DIRS = ("/System/Applications", "/System/Applications/Utilities", "/Applications")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def key(path):
    text = str(Path(path).resolve())
    return "~" + text[len(HOME):] if text.startswith(HOME + "/") else text


def harness_files():
    out = []
    for p in sorted(HELDOUT.rglob("*")):
        rel = p.relative_to(HELDOUT).parts
        if not p.is_file() or rel[0] in ("runs", "tests") or "__pycache__" in rel or p.suffix == ".pyc":
            continue
        if p.suffix in (".py", ".sh", ".zsh") or (len(rel) >= 3 and rel[:2] == ("arms", "bin")):
            out.append(p)
    for name in ("driver.py", "pricing.py", "tasks.py", "lockq.sh", "macctl.swift", "out/macctl"):
        if (BENCH / name).is_file():
            out.append(BENCH / name)
    return out


def wrapper_targets(wrapper, tools):
    text = Path(wrapper).read_text(errors="replace")
    out = []
    for m in EXEC_PATH.finditer(text):
        base = tools if m.group(0).lstrip('"').startswith("$T") else Path(HOME)
        p = Path(str(base) + m.group(1))
        if p.is_file() and os.access(p, os.X_OK):
            out.append(p)
            helper = p.with_name("agent-desktop-macos-helper")
            if helper.is_file():
                out.append(helper)
    return out


def app_version(app):
    for d in APP_DIRS:
        info = Path(d) / f"{app}.app" / "Contents" / "Info.plist"
        if info.is_file():
            d = plistlib.loads(info.read_bytes())
            return f"{d.get('CFBundleShortVersionString')} ({d.get('CFBundleVersion')})"
    return None


def run(*cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=30).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return None


def build(taskset, arms):
    sys.path.insert(0, str(HELDOUT / "arms"))
    os.environ["HELDOUT_TASKSET"] = str(taskset)
    import common
    th = common.load_taskset()
    files = {}
    for p in harness_files():
        files[key(p)] = sha(p)
    root = Path(taskset).parent
    pinned = re.findall(r"`([^`]+)` sha256 `[0-9a-f]{64}`", (root / "FROZEN.md").read_text())
    for name in pinned + ["schedule.json"]:
        if (root / name).is_file():
            files[key(root / name)] = sha(root / name)
    files[key(common.VERIFY_AD)] = sha(common.VERIFY_AD)
    prompts = {}
    for arm in arms:
        bin_dir = common.arm_bin_dir(arm)
        for w in sorted(bin_dir.iterdir()):
            if w.is_file():
                files[key(w)] = sha(w)
                for target in wrapper_targets(w, common.TOOLS):
                    files[key(target)] = sha(target)
        if arm in common.CANDIDATE_ARMS:
            for name in ("agent-desktop", "agent-desktop-macos-helper"):
                if (common.CANDIDATES / arm / name).is_file():
                    files[key(common.CANDIDATES / arm / name)] = sha(common.CANDIDATES / arm / name)
        if arm == "cua":
            import cua_daemon
            for f in sorted((cua_daemon.APP / "Contents").rglob("*")):
                if f.is_file() and not f.is_symlink():
                    files[key(f)] = sha(f)
        guide = common.NATIVE_GUIDE_DIR.get(arm)
        if guide is not None:
            for g in sorted(guide.rglob("*")):
                if g.is_file():
                    files[key(g)] = sha(g)
        prompts[arm] = hashlib.sha256(common.native_system_prompt(arm).encode()).hexdigest()
    settings = {"claude": run("claude", "--version"), "rules": common.RULES, "cap_s": th.NATIVE_CAP_S,
                "macos": f"{run('sw_vers', '-productVersion')} ({run('sw_vers', '-buildVersion')})",
                "apps": {app: app_version(app) for app in getattr(th, "NATIVE_APPS", [])}}
    return {"files": files, "prompts": prompts, "settings": settings}


def digest(manifest):
    return hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()


def main():
    args = sys.argv[1:]
    only = args[:1] == ["--digest"]
    taskset, arms = Path(args[1 if only else 0]), args[2 if only else 1:]
    m = build(taskset, arms)
    print(digest(m) if only else json.dumps(dict(m, digest=digest(m)), indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
