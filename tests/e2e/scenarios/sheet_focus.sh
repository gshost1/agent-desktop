note "A sheet stays listed while another window of the app has focus"

focused_window_title() {
    "$bin" list-windows --app "$app" 2>/dev/null | "$e2e_python" -c '
import json, sys
pid = int(sys.argv[1])
focused = [w.get("title", "") for w in json.load(sys.stdin).get("data", [])
           if w.get("pid") == pid and w.get("is_focused")]
print(focused[0] if len(focused) == 1 else "")
' "$fixture_pid" 2>/dev/null
}

sheet_listed() {
    "$bin" list-surfaces --app "$app" 2>/dev/null | grep -q '"type":"sheet"' && echo 1 || echo 0
}

sheet_focus_suite() {
    local open_panel panel open_sheet panel_text focused listed confirm close
    "$bin" focus-window --app "$app" >/dev/null 2>&1
    require_target open_panel button open-floating-panel
    act_target "$open_panel" scroll-to >/dev/null 2>&1
    require_target open_panel button open-floating-panel
    act_target "$open_panel" click >/dev/null 2>&1
    panel="$(await_panel_window)"
    [ -n "$panel" ] || { assert "the floating panel opens for the sheet focus check" 0 ""; return; }
    require_target open_sheet button open-sheet
    act_target "$open_sheet" scroll-to >/dev/null 2>&1
    require_target open_sheet button open-sheet
    act_target "$open_sheet" click >/dev/null 2>&1
    sleep 0.6
    assert "list-surfaces reports the sheet while its window has focus" "$(sheet_listed)" ""
    panel_text="$(panel_target "$panel" textfield panel-text)"
    act_target "$panel_text" focus >/dev/null 2>&1
    for _ in $(seq 1 25); do
        focused="$(focused_window_title)"
        [ "$focused" = "Fixture Panel" ] && break
        sleep 0.1
    done
    assert "focusing a field in the panel moves the app's focused window to the panel" \
        "$([ "$focused" = "Fixture Panel" ] && echo 1 || echo 0)" "focused=$focused"
    listed="$(sheet_listed)"
    assert "list-surfaces still reports the sheet of a window that lost focus" "$listed" \
        "focused=$focused"
    require_target confirm button confirm-sheet
    act_target "$confirm" click >/dev/null 2>&1
    close="$(panel_target "$panel" button Close)"
    act_target "$close" click >/dev/null 2>&1
    for _ in $(seq 1 25); do
        [ -z "$(panel_window_id)" ] && break
        sleep 0.1
    done
}

sheet_focus_suite
