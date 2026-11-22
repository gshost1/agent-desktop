fixture_window_id() {
    "$bin" list-windows --app "$app" 2>/dev/null | "$e2e_python" -c '
import json, sys
pid, title = int(sys.argv[1]), sys.argv[2]
windows = [w for w in json.load(sys.stdin).get("data", [])
           if w.get("pid") == pid and w.get("title") == title]
print(windows[0]["id"] if len(windows) == 1 else "")
' "$fixture_pid" "$1" 2>/dev/null
}

written_report() {
    printf '%s/%s/%s' "$(json_field "$1" ok)" "$(json_field "$1" data.disposition.delivery)" \
        "$(json_field "$1" data.details.application_commit)"
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
    local check_status width_status selection_status repeated code delivery commit commit_status written
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

    commit="$(panel_target "$window" textfield panel-commit)"
    written="$(act_target "$commit" set-value "committed-$label" 2>/dev/null)"
    assert "set-value on a field reports that the app's commit is unobserved" \
        "$([ "$(json_field "$written" data.disposition.delivery)" = "delivered_unverified" ] && \
            [ "$(json_field "$written" data.details.application_commit)" = "unobserved" ] && echo 1 || echo 0)" \
        "delivery=$(json_field "$written" data.disposition.delivery) commit=$(json_field "$written" data.details.application_commit)"
    height="$(panel_target "$window" textfield panel-height)"
    act_target "$height" focus >/dev/null 2>&1
    commit_status="$(main_window_value panel-commit-status "committed-$label")"
    assert "focus on another field ends the edit and the app commits it" \
        "$([ "$commit_status" = "committed-$label" ] && echo 1 || echo 0)" "committed=$commit_status"

    text="$(panel_target "$window" textfield panel-commit-text)"
    written="$(act_target "$text" set-value "view-$label" 2>/dev/null)"
    height="$(panel_target "$window" textfield panel-height)"
    act_target "$height" focus >/dev/null 2>&1
    commit_status="$(main_window_value panel-commit-text-status "view-$label")"
    assert "set-value on a text view reports an unobserved commit that focus elsewhere ends" \
        "$([ "$(written_report "$written")" = "True/delivered_unverified/unobserved" ] && \
            [[ "$(json_field "$written" data.details.commit_hint)" == *'check the saved result'* ]] && \
            [ "$commit_status" = "view-$label" ] && echo 1 || echo 0)" \
        "report=$(written_report "$written") committed=$commit_status"

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

form_commit_counts() {
    printf '%s/%s/%s/%s' "$(main_window_value form-action-count "")" \
        "$(main_window_value form-default-count "")" "$(main_window_value swift-default-count "")" \
        "$(main_window_value swift-submit-count "")"
}

form_commit_deltas() {
    local a b c d w x y z
    [[ "$1" =~ ^[0-9]+/[0-9]+/[0-9]+/[0-9]+$ && "$2" =~ ^[0-9]+/[0-9]+/[0-9]+/[0-9]+$ ]] || return 1
    IFS=/ read -r a b c d <<< "$1"
    IFS=/ read -r w x y z <<< "$2"
    printf '%s/%s/%s/%s' "$((w - a))" "$((x - b))" "$((y - c))" "$((z - d))"
}

commit_delivery_report() {
    local section=data
    [ "$(json_field "$1" ok)" = "True" ] || section=error
    printf '%s/%s/%s/%s' "$(json_field "$1" ok)" "$(json_field "$1" "$section.disposition.delivery")" \
        "$(json_field "$1" "$section.details.kind")$(json_field "$1" "$section.details.application_commit" | sed 's/^unobserved$//')" \
        "$(json_field "$1" "$section.details.confirm_sent")"
}

form_suite() {
    local open_form form target written report action_count search_count plain_status counts action_before counts_before deltas mechanism submit_delta
    note "[form] set-value never presses a default button or submits, and reports what it cannot see"
    "$bin" focus-window --app "$app" >/dev/null 2>&1
    main_window="$(fixture_window_id "AgentDesk Fixture")"
    require_target open_form button open-form-window
    act_target "$open_form" scroll-to >/dev/null 2>&1
    require_target open_form button open-form-window
    act_target "$open_form" click >/dev/null 2>&1
    for _ in $(seq 1 25); do
        form="$(fixture_window_id "Fixture Form")"
        [ -n "$form" ] && break
        sleep 0.1
    done
    [ -n "$form" ] || { assert "the form window opens" 0 ""; return; }

    target="$(panel_target "$form" textfield form-action)"
    written="$(act_target "$target" set-value "written-action" 2>/dev/null)"
    report="$(written_report "$written")"
    action_count="$(main_window_value form-action-count 0)"
    assert "set-value on a field with an action reports an unobserved commit and runs no action" \
        "$([ "$report" = "True/delivered_unverified/unobserved" ] && [ "$action_count" = "0" ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report action=$action_count"

    target="$(panel_target "$form" textfield form-search)"
    written="$(act_target "$target" set-value "written-search" 2>/dev/null)"
    report="$(written_report "$written")"
    search_count="$(main_window_value form-search-count 0)"
    assert "set-value on a search field is settled by its read-back and runs no search action" \
        "$([ "$report" = "True/delivered_verified/not_verified" ] && [ "$search_count" = "0" ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report search=$search_count"

    target="$(panel_target "$form" textfield form-plain)"
    written="$(act_target "$target" set-value "written-plain" 2>/dev/null)"
    report="$(written_report "$written")"
    plain_status="$(main_window_value form-plain-status plain)"
    assert "set-value on a commit-on-end-editing field leaves the commit to the caller" \
        "$([ "$report" = "True/delivered_unverified/unobserved" ] && [ "$plain_status" = "plain" ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report committed=$plain_status"

    require_target target textfield swift-submit
    written="$(act_target "$target" set-value "written-swift" 2>/dev/null)"
    report="$(written_report "$written")"
    counts="$(main_window_value form-default-count 0)/$(main_window_value swift-default-count 0)/$(main_window_value swift-submit-count 0)"
    assert "set-value presses no default button and runs no SwiftUI submit" \
        "$([ "$report" = "True/delivered_unverified/unobserved" ] && [ "$counts" = "0/0/0" ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report default/swift-default/swift-submit=$counts"

    target="$(panel_target "$form" textfield form-search)"
    act_target "$target" focus >/dev/null 2>&1
    plain_status="$(main_window_value form-plain-status written-plain)"
    assert "focus on another field in the window commits the written field" \
        "$([ "$plain_status" = "written-plain" ] && echo 1 || echo 0)" "committed=$plain_status"

    target="$(panel_target "$form" textfield form-plain)"
    written="$(act_target "$target" clear 2>/dev/null)"
    report="$(written_report "$written")"
    plain_status="$(main_window_value form-plain-status written-plain)"
    assert "clear on a commit-on-end-editing field leaves the commit to the caller like set-value" \
        "$([ "$report" = "True/delivered_unverified/unobserved" ] && [ "$plain_status" = "written-plain" ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report committed=$plain_status"
    "$bin" focus-window --window-id "$form" >/dev/null 2>&1
    target="$(panel_target "$form" textfield form-plain)"
    counts_before="$(form_commit_counts)"
    written="$(act_target "$target" set-value "committed-plain" --commit 2>/dev/null)"
    report="$(written_report "$written")"
    plain_status="$(main_window_value form-plain-status committed-plain)"
    counts="$(form_commit_counts)"
    deltas="$(form_commit_deltas "$counts_before" "$counts")"
    mechanism="$(json_field "$written" data.details.commit_evidence.mechanism)"
    assert "set-value --commit commits the plain field without another action or default button" \
        "$([ "$report" = "True/delivered_verified/committed" ] && [ "$plain_status" = "committed-plain" ] && \
            [ "$deltas" = "0/0/0/0" ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report committed=$plain_status mechanism=$mechanism action/default/swift-default/swift-submit=$counts_before->$counts deltas=$deltas"

    target="$(panel_target "$form" textfield form-action)"
    counts_before="$(form_commit_counts)"
    action_before="${counts_before%%/*}"
    written="$(act_target "$target" set-value "committed-action" --commit 2>/dev/null)"
    report="$(commit_delivery_report "$written")"
    action_count="$(main_window_value form-action-count "$((action_before + 1))")"
    counts="$(form_commit_counts)"
    deltas="$(form_commit_deltas "$counts_before" "$counts")"
    assert "set-value --commit on a field with an action runs only its own action" \
        "$([ "$report" = "False/delivered_unverified/edit_commit_no_safe_peer/True" ] && \
            [ "$deltas" = "1/0/0/0" ] && \
            [ "$action_count" = "$((action_before + 1))" ] && echo 1 || echo 0)" \
        "ok/delivery/kind/confirm_sent=$report action/default/swift-default/swift-submit=$counts_before->$counts deltas=$deltas"

    "$bin" focus-window --window-id "$main_window" >/dev/null 2>&1
    require_target target textfield swift-submit
    counts_before="$(form_commit_counts)"
    written="$(act_target "$target" set-value "committed-swift" --commit 2>/dev/null)"
    report="$(written_report "$written")"
    counts="$(form_commit_counts)"
    deltas="$(form_commit_deltas "$counts_before" "$counts")"
    submit_delta="${deltas##*/}"
    mechanism="$(json_field "$written" data.details.commit_evidence.mechanism)"
    assert "set-value --commit on SwiftUI runs at most its own submit and no default button" \
        "$([ "$report" = "True/delivered_verified/committed" ] && [ "${deltas%/*}" = "0/0/0" ] && \
            [ "$submit_delta" -ge 0 ] && [ "$submit_delta" -le 1 ] && echo 1 || echo 0)" \
        "ok/delivery/commit=$report mechanism=$mechanism action/default/swift-default/swift-submit=$counts_before->$counts deltas=$deltas submit_delta=$submit_delta"
    act_target "$(panel_target "$form" button Close)" click >/dev/null 2>&1
}

form_suite
