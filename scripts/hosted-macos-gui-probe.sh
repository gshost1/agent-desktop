#!/usr/bin/env bash
#
# Probes whether a GitHub-hosted macOS runner can drive real GUI apps through
# agent-desktop. Subcommands: env, grant, display, tiny. Each one writes to $PROBE_LOGS.
# It is meant only for disposable hosted runners: `grant` writes TCC.db rows.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${PROBE_BIN:-$ROOT/target/release/agent-desktop}"
LOGS="${PROBE_LOGS:-${RUNNER_TEMP:-/tmp}/gui-probe-logs}"
USER_TCC="$HOME/Library/Application Support/com.apple.TCC/TCC.db"
SYSTEM_TCC="/Library/Application Support/com.apple.TCC/TCC.db"
SERVICES="kTCCServiceAccessibility kTCCServiceScreenCapture kTCCServicePostEvent"
mkdir -p "$LOGS"

if [ "${GITHUB_ACTIONS:-}" != "true" ] || [ "${RUNNER_ENVIRONMENT:-}" != "github-hosted" ]; then
    echo "refusing: this probe writes TCC.db and runs only on a GitHub-hosted runner" >&2
    exit 2
fi

section() {
    printf '\n===== %s =====\n' "$1"
}

ancestor_paths() {
    local pid="$$" exe
    while [ -n "$pid" ] && [ "$pid" -gt 1 ]; do
        exe="$(ps -o comm= -p "$pid" 2>/dev/null)"
        case "$exe" in
            /*) printf '%s\n' "$exe" ;;
            *) command -v "$exe" 2>/dev/null || printf '%s\n' "$exe" ;;
        esac
        pid="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')"
    done
}

tcc_rows() {
    local db="$1" prefix="$2"
    $prefix sqlite3 "$db" "SELECT service, client, client_type, auth_value FROM access WHERE service IN ('kTCCServiceAccessibility','kTCCServiceScreenCapture','kTCCServicePostEvent','kTCCServiceAppleEvents') ORDER BY service, client;" 2>&1
}

report_env() {
    section "sw_vers"; sw_vers; uname -m
    section "csrutil status"; csrutil status 2>&1
    section "identity"; id; echo "HOME=$HOME"
    section "console owner"; stat -f %Su /dev/console 2>&1
    section "gui domain"; launchctl print "gui/$(id -u)" 2>&1 | head -40
    section "loginwindow and WindowServer"; pgrep -lx loginwindow; pgrep -lx WindowServer
    section "ancestor chain"; ancestor_paths
    section "process tree"; ps -axo pid,ppid,user,comm | grep -Ei 'runner|provisioner|launchd|ankarun|bash|login' | head -40
    section "user TCC.db"; tcc_rows "$USER_TCC" ""
    section "system TCC.db"; tcc_rows "$SYSTEM_TCC" "sudo"
    section "system TCC schema"; sudo sqlite3 "$SYSTEM_TCC" ".schema access" 2>&1
    section "tccutil"; tccutil 2>&1 | head -5 || true
}

permission_states() {
    "$BIN" permissions 2>&1 | /usr/bin/python3 -c '
import json, sys
raw = sys.stdin.read()
try:
    data = json.loads(raw)["data"]
    print(" ".join(k + "=" + str(data.get(k, {}).get("state")) for k in ("accessibility", "screen_recording", "automation")))
except Exception:
    print("unparsed: " + raw.strip())
'
}

insert_rows() {
    local db="$1" prefix="$2" client="$3" service now
    now="$(date +%s)"
    for service in $SERVICES; do
        $prefix sqlite3 "$db" "INSERT OR REPLACE INTO access (service, client, client_type, auth_value, auth_reason, auth_version, indirect_object_identifier, flags, last_modified) VALUES ('$service', '$client', 1, 2, 4, 1, 'UNUSED', 0, $now);" 2>&1 \
            && echo "inserted $service for $client" || echo "insert failed: $service for $client"
    done
}

grant_clients() {
    printf '%s\n' "$BIN"
    ancestor_paths | grep '^/' | sort -u
}

restart_tcc() {
    killall tccd 2>&1 || true
    sudo killall tccd 2>&1 || true
    sleep 2
}

try_grants() {
    local client
    section "baseline"; permission_states
    section "method 1: user TCC.db rows"
    grant_clients | while IFS= read -r client; do insert_rows "$USER_TCC" "" "$client"; done
    restart_tcc
    permission_states
    section "method 2: system TCC.db rows with sudo"
    grant_clients | while IFS= read -r client; do insert_rows "$SYSTEM_TCC" "sudo" "$client"; done
    restart_tcc
    permission_states
    section "method 3: tccutil"
    echo "tccutil exposes only reset; it cannot grant a service"
    section "after grants: system TCC.db"; tcc_rows "$SYSTEM_TCC" "sudo"
    case "$(permission_states)" in
        *accessibility=granted*) echo "RESULT accessibility=granted"; return 0 ;;
        *) echo "RESULT accessibility=not-granted"; return 1 ;;
    esac
}

json_get() {
    /usr/bin/python3 -c '
import json, sys
node = json.load(sys.stdin)
for part in sys.argv[1].split("."):
    node = node.get(part) if isinstance(node, dict) else None
print("" if node is None else node)
' "$1"
}

find_ref() {
    /usr/bin/python3 -c '
import json, sys
want = sys.argv[1]
def walk(node):
    if node.get("ref_id") and node.get("role") == "button" and want in (node.get("name"), node.get("description")):
        return node["ref_id"]
    for child in node.get("children", []):
        hit = walk(child)
        if hit:
            return hit
    return None
print(walk(json.load(sys.stdin)["data"]["tree"]) or "")
' "$1"
}

tree_texts() {
    /usr/bin/python3 -c '
import json, sys
out = []
def walk(node):
    for key in ("name", "value"):
        if isinstance(node.get(key), str):
            out.append(node[key])
    for child in node.get("children", []):
        walk(child)
walk(json.load(sys.stdin)["data"]["tree"])
print("\n".join(out))
'
}

display_mode() {
    section "display before"; "$BIN" list-displays 2>&1
    section "system_profiler"; system_profiler SPDisplaysDataType 2>&1 | head -30
    section "CoreGraphics modes"
    swift "$ROOT/scripts/hosted-macos-display-mode.swift" 1280 900 2>&1
    local status=$?
    sleep 2
    section "display after"; "$BIN" list-displays 2>&1
    return "$status"
}

tiny_check() {
    local snap ref after
    section "list-displays"; "$BIN" list-displays 2>&1
    section "launch Calculator"; "$BIN" launch Calculator 2>&1 | head -c 2000; echo
    section "snapshot"
    snap="$("$BIN" snapshot --app Calculator -i 2>&1)"
    printf '%s\n' "$snap" > "$LOGS/calculator-snapshot.json"
    printf '%s' "$snap" | json_get ok
    for name in 7 Seven; do
        ref="$(printf '%s' "$snap" | find_ref "$name" 2>/dev/null)"
        [ -n "$ref" ] && break
    done
    if [ -z "$ref" ]; then
        echo "TINY FAIL: no button named 7 in the Calculator snapshot"
        head -c 3000 "$LOGS/calculator-snapshot.json"
        return 1
    fi
    section "click $ref"
    "$BIN" click "$ref" 2>&1 | tee "$LOGS/calculator-click.json"
    section "read back"
    after="$("$BIN" snapshot --app Calculator 2>&1)"
    printf '%s\n' "$after" > "$LOGS/calculator-after.json"
    if printf '%s' "$after" | tree_texts | grep -Eq '^(.*[^0-9])?7$'; then
        echo "TINY PASS: Calculator shows 7 after the click"
        "$BIN" close-app Calculator >/dev/null 2>&1
        return 0
    fi
    echo "TINY FAIL: the display does not show 7 after the click"
    printf '%s' "$after" | tree_texts | head -40
    "$BIN" close-app Calculator >/dev/null 2>&1
    return 1
}

case "${1:-}" in
    env) report_env 2>&1 | tee "$LOGS/env.log"; exit 0 ;;
    grant) try_grants 2>&1 | tee "$LOGS/grant.log"; exit "${PIPESTATUS[0]}" ;;
    display) display_mode 2>&1 | tee "$LOGS/display.log"; exit "${PIPESTATUS[0]}" ;;
    tiny) tiny_check 2>&1 | tee "$LOGS/tiny.log"; exit "${PIPESTATUS[0]}" ;;
    *) echo "usage: $0 env|grant|display|tiny" >&2; exit 2 ;;
esac
