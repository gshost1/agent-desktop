"""Start a held-out run's Cua Driver daemon as CuaDriver.app, the identity that holds the Accessibility grant.

A daemon the CLI starts itself (`cua-driver serve`) runs as the bare CLI and gets `permissions_pending` on every call.
All 42 cua runs of the first pass after the per-run stop failed that way (fresh-comp, 2026-11-15), while the 120
earlier cua runs had found a granted daemon already up. Each run therefore starts its own daemon from the app with
`open`, checks the grant before the model starts, and stops it afterwards (run_native.post_arm_cleanup).
"""
import json
import os
import subprocess
import time
from pathlib import Path

import common

APP = common.TOOLS / "cua-driver-0.47.7" / "cua-driver-rs-0.47.7-darwin-arm64" / "CuaDriver.app"
POLLS = 40


def launch(app, home, tool_home=None):
    """Open the app as a new instance in the background, with telemetry off, this run's driver home, and HOME at the
    run's tool home (logwrap.zsh gives the CLI the same HOME, so both find the daemon's socket under it and nothing
    is written to the host account's Library)."""
    subprocess.run(["open", "-n", "-g", "--env", "CUA_DRIVER_RS_TELEMETRY_ENABLED=0", "--env",
                    "CUA_TELEMETRY_ENABLED=false", "--env", f"CUA_DRIVER_RS_HOME={home}", "--env",
                    f"CUA_DRIVER_TELEMETRY_HOME={home / 'telemetry'}", "--env", f"HOME={tool_home or home.parent / 'home'}", str(app),
                    "--args", "serve", "--no-overlay"],
                   capture_output=True, timeout=30)


def probe_with(cli, arm_home, tool_home=None):
    def probe(args):
        env = dict(os.environ, HO_ARM_HOME=str(arm_home), **({"HO_TOOL_HOME": str(tool_home)} if tool_home else {}))
        return subprocess.run([cli, *args], env=env, capture_output=True, text=True, timeout=30)
    return probe


def start(arm_home, cli, app=APP, opener=launch, prober=None, sleep=time.sleep, tool_home=None):
    """None once the daemon runs and reports its Accessibility grant, else the infrastructure fault."""
    arm_home = Path(arm_home)
    home = arm_home / "cua-home"
    (home / "telemetry").mkdir(parents=True, exist_ok=True)
    prober = prober or probe_with(cli, arm_home, tool_home)
    if not Path(app).exists():
        return f"CuaDriver.app is missing at {app}"
    opener(app, home) if tool_home is None else opener(app, home, tool_home)
    for _ in range(POLLS):
        if prober(["status"]).returncode == 0:
            break
        sleep(0.5)
    else:
        return "the CuaDriver.app daemon did not start"
    try:
        perms = json.loads(prober(["permissions", "status", "--json"]).stdout or "{}")
    except ValueError:
        perms = {}
    if perms.get("accessibility") is True and perms.get("daemon_running") is True:
        return None
    shown = {k: perms.get(k) for k in ("accessibility", "daemon_running", "status", "reason") if k in perms}
    return f"the cua daemon lacks its Accessibility grant: {json.dumps(shown)[:200]}"
