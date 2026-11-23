#!/usr/bin/env python3
"""Check that a renamed copy of the CLI starts the macOS cursor overlay renderer.

The renderer is the CLI started again with a marker, so its file name must not
matter: `session start --cursor` from a copy named `deskref` reports
`rendered: true`, a renderer process holds the session's socket, and
`session end` removes both.
"""
import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    state_directory = tempfile.TemporaryDirectory(prefix="adr-", dir="/tmp")
    state = Path(state_directory.name)
    renamed = state / "bin" / "deskref"
    renamed.parent.mkdir()
    shutil.copy2(args.bin.resolve(), renamed)
    env = dict(os.environ, AGENT_DESKTOP_HOME=str(state))
    env.pop("AGENT_DESKTOP_SESSION", None)
    env.pop("AGENT_DESKTOP_AGENT_ID", None)

    def cli(*command):
        result = subprocess.run([str(renamed), *command], env=env, capture_output=True,
                                text=True, timeout=15)
        value = json.loads(result.stdout)
        assert result.returncode == 0 and value["ok"], value
        return value["data"]

    def sockets():
        return sorted(state.glob(".cursor-overlay-*.sock"))

    def alive(pid):
        try:
            os.kill(pid, 0)
            return True
        except ProcessLookupError:
            return False

    session_id = None
    pids = []
    try:
        started = cli("session", "start", "--cursor")
        session_id = started["session_id"]
        assert started.get("rendered") is True, started
        socket, = sockets()
        pids = [int(pid) for pid in subprocess.check_output(["lsof", "-t", str(socket)],
                                                            text=True).split()]
        assert pids, "no renderer holds the cursor socket"
        command = subprocess.check_output(["ps", "-o", "comm=", "-p", str(pids[0])],
                                          text=True).strip()
        assert Path(command).name == "deskref", command
        cli("session", "end", session_id)
        session_id = None
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline and (sockets() or any(map(alive, pids))):
            time.sleep(0.05)
        assert not sockets(), "session end left the cursor socket"
        assert not any(map(alive, pids)), "renderer survived session end"
        report = {"renamed_binary": renamed.name, "rendered": True, "renderer_pids": pids,
                  "renderer_command": command, "cleanup_verified": True}
        (args.out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
    finally:
        if session_id:
            subprocess.run([str(renamed), "session", "end", session_id], env=env,
                           capture_output=True, timeout=15)
        for pid in pids:
            if alive(pid):
                os.kill(pid, 9)
        state_directory.cleanup()


if __name__ == "__main__":
    main()
