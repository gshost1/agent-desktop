safety_checkpoint || fail "ownership checkpoint failed before the background chord"
chord_output="$(agent_exec 1 press cmd+j --app "$app" --window-id "$fixture_window_id" \
    2>"$suite_root/background-chord.stderr")"
chord_exit=$?
printf '%s' "$chord_output" > "$suite_root/background-chord.json"
[ "$chord_exit" -eq 1 ] || fail "a background command chord must return a structured error (exit=$chord_exit): $chord_output"
[ "$(json_field "$chord_output" error.code)" = "ACTION_FAILED" ] \
    || fail "background command chord error code is incorrect: $chord_output"
[ "$(json_field "$chord_output" error.details.kind)" = "command_chord_unrouted_in_background" ] \
    || fail "background command chord was not refused as unrouted: $chord_output"
[ "$(json_field "$chord_output" error.details.physical_delivery_started)" = "False" ] \
    || fail "background command chord must report that no keystroke was posted: $chord_output"
[ "$(json_field "$chord_output" error.disposition.delivery)" = "not_delivered" ] \
    || fail "background command chord must be not_delivered: $chord_output"
safety_checkpoint || fail "ownership checkpoint failed after the background chord"
pass "a command chord no menu item takes is refused for a background app, not reported as sent"
