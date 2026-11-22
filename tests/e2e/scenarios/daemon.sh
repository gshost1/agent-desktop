daemon_state_field() {
    local field="$1"
    "$e2e_python" - "$HOME/.agent-desktop/daemon" "$field" <<'PY' 2>/dev/null
import json, pathlib, sys
states = sorted(pathlib.Path(sys.argv[1]).glob("*.state"))
if len(states) == 1:
    print(json.loads(states[0].read_text())[sys.argv[2]])
PY
}

daemon_exec() {
    guard_exec 30 4194304 env AGENT_DESKTOP_DAEMON=1 AGENT_DESKTOP_DAEMON_IDLE_MS=120000 \
        "$raw_bin" "$@"
}

daemon_suite() {
    note "daemon mode: read-only commands through a warm daemon against the fixture"
    local warm pid perm direct through before after direct_snapshot daemon_snapshot sockets
    warm="$(daemon_exec version 2>/dev/null)"
    pid="$(daemon_state_field pid)"
    assert "the first daemon-mode call starts one daemon and runs in it" \
        "$([ "$(json_field "$warm" ok)" = "True" ] && [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && [ "$(daemon_state_field served)" = "1" ] && echo 1 || echo 0)" \
        "ok=$(json_field "$warm" ok) pid='$pid' served='$(daemon_state_field served)'"
    if [ -z "$pid" ]; then
        return
    fi
    perm="$(daemon_exec permissions 2>/dev/null)"
    assert "the daemon itself holds the accessibility grant" \
        "$([ "$(json_field "$perm" data.accessibility.state)" = "granted" ] && echo 1 || echo 0)" \
        "state=$(json_field "$perm" data.accessibility.state)"
    require_value direct click-status
    before="$(daemon_state_field served)"
    through="$(daemon_exec find --app "$app" --role statictext --native-id click-status --first 2>/dev/null \
        | "$e2e_python" "$json_tool" match-value 2>/dev/null)"
    after="$(daemon_state_field served)"
    assert "find through the daemon reads the value the fixture shows" \
        "$([ -n "$through" ] && [ "$through" = "$direct" ] && [ "$after" = "$((before + 1))" ] && echo 1 || echo 0)" \
        "direct='$direct' daemon='$through' served $before->$after"
    direct_snapshot="$("$bin" snapshot --app "$app" -i 2>/dev/null)"
    daemon_snapshot="$(daemon_exec snapshot --app "$app" -i 2>/dev/null)"
    assert "a snapshot through the daemon sees the same interactive tree" \
        "$([ "$(json_field "$daemon_snapshot" ok)" = "True" ] && [ "$(json_field "$daemon_snapshot" data.complete)" = "True" ] && [ "$(json_field "$daemon_snapshot" data.ref_count)" = "$(json_field "$direct_snapshot" data.ref_count)" ] && echo 1 || echo 0)" \
        "direct ref_count=$(json_field "$direct_snapshot" data.ref_count) daemon ref_count=$(json_field "$daemon_snapshot" data.ref_count) complete=$(json_field "$daemon_snapshot" data.complete)"
    kill -TERM "$pid" 2>/dev/null
    for _ in $(seq 1 50); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    sockets="$(find "$HOME/.agent-desktop/daemon" -name '*.sock' 2>/dev/null)"
    assert "a stopped daemon leaves no process and no socket" \
        "$(! kill -0 "$pid" 2>/dev/null && [ -z "$sockets" ] && echo 1 || echo 0)" \
        "pid=$pid sockets='$sockets'"
}

daemon_suite
