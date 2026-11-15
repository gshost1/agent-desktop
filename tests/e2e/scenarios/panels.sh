fixture_window_id() {
    "$bin" list-windows --app "$app" 2>/dev/null | "$e2e_python" -c '
import json, sys
pid, title = int(sys.argv[1]), sys.argv[2]
windows = [w for w in json.load(sys.stdin).get("data", [])
           if w.get("pid") == pid and w.get("title") == title]
print(windows[0]["id"] if len(windows) == 1 else "")
' "$fixture_pid" "$1" 2>/dev/null
}

panel_window_id() { fixture_window_id "Fixture Panel"; }

main_window_value() {
    local name="$1" expected="$2" output value=""
    for _ in $(seq 1 25); do
        output="$("$bin" find --window-id "$main_window" --role statictext --native-id "$name" --first 2>/dev/null)"
        value="$(printf '%s' "$output" | "$e2e_python" "$json_tool" match-value 2>/dev/null)"
        [ -n "$expected" ] && [ "$value" = "$expected" ] && break
        [ -z "$expected" ] && [ -n "$value" ] && break
        sleep 0.2
    done
    printf '%s' "$value"
}

await_panel_window() {
    local id=""
    for _ in $(seq 1 25); do
        id="$(panel_window_id)"
        [ -n "$id" ] && break
        sleep 0.1
    done
    printf '%s' "$id"
}

panel_target() {
    local window="$1" role="$2" name="$3"
    "$bin" find --window-id "$window" --role "$role" --name "$name" --first 2>/dev/null | \
        "$e2e_python" "$json_tool" target 2>/dev/null
}

panel_suite() {
    local label="$1" open_panel window main_window chosen check height text close
    local check_status width_status selection_status repeated code delivery
    MODE_FLAG="$2"
    note "[$label] floating panel: listed, observed, acted on"
    "$bin" focus-window --app "$app" >/dev/null 2>&1
    main_window="$(fixture_window_id "AgentDesk Fixture")"
    [ -n "$main_window" ] || { assert "list-windows lists the fixture's main window" 0 ""; return; }
    require_target open_panel button open-floating-panel
    act_target "$open_panel" scroll-to >/dev/null 2>&1
    require_target open_panel button open-floating-panel
    act_target "$open_panel" click >/dev/null 2>&1
    window="$(await_panel_window)"
    assert "list-windows lists the app's floating panel" \
        "$([ -n "$window" ] && echo 1 || echo 0)" "window=$window"
    [ -n "$window" ] || return
    chosen="$(json_field "$("$bin" snapshot --app "$app" -i --compact 2>/dev/null)" data.window.title)"
    assert "snapshot --app keeps the focused document window while a panel floats" \
        "$([ "$chosen" = "AgentDesk Fixture" ] && echo 1 || echo 0)" "window=$chosen"

    check="$(panel_target "$window" checkbox "Panel Check")"
    act_target "$check" click >/dev/null 2>&1
    check_status="$(main_window_value panel-check-status on)"
    assert "a ref from the panel's own snapshot acts in the panel" \
        "$([ "$check_status" = "on" ] && echo 1 || echo 0)" "status=$check_status"

    height="$(panel_target "$window" textfield panel-height)"
    act_target "$height" set-value 512 >/dev/null 2>&1
    width_status="$(main_window_value panel-width-status 768)"
    assert "set-value on a text field reaches the app's own edit handling" \
        "$([ "$width_status" = "768" ] && echo 1 || echo 0)" "linked width=$width_status"

    text="$(panel_target "$window" textfield panel-text)"
    act_target "$text" select gamma >/dev/null 2>&1
    selection_status="$(main_window_value panel-selection-status 11+5)"
    assert "select on a text element selects the text's one occurrence" \
        "$([ "$selection_status" = "11+5" ] && echo 1 || echo 0)" "selection=$selection_status"
    text="$(panel_target "$window" textfield panel-text)"
    repeated="$(act_target "$text" select beta 2>/dev/null)"
    code="$(json_field "$repeated" error.code)"
    delivery="$(json_field "$repeated" error.disposition.delivery)"
    selection_status="$(main_window_value panel-selection-status "")"
    assert "select refuses a text that occurs twice and leaves the selection" \
        "$([ "$code" = "AMBIGUOUS_TARGET" ] && [ "$delivery" = "not_delivered" ] && \
            [ "$selection_status" = "11+5" ] && echo 1 || echo 0)" \
        "code=$code delivery=$delivery selection=$selection_status"

    act_target "$check" click >/dev/null 2>&1
    main_window_value panel-check-status off >/dev/null
    close="$(panel_target "$window" button Close)"
    act_target "$close" click >/dev/null 2>&1
    for _ in $(seq 1 25); do
        [ -z "$(panel_window_id)" ] && break
        sleep 0.1
    done
    assert "closing the panel removes it from list-windows" \
        "$([ -z "$(panel_window_id)" ] && echo 1 || echo 0)" "window=$(panel_window_id)"
    MODE_FLAG=""
}

panel_suite headless ""
check_fixture_contention || { finish; exit 1; }
panel_suite headed "--headed"
