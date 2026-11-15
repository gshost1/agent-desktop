note "Wide fixture window: plain click on content narrower than the window"
wide_width=1300
wide_frame="$(fixture_frame "$wide_width")"
wide_status=$?
if [ "$wide_status" -eq 3 ]; then
    skipmsg "wide-window click unavailable: the largest display is narrower than $((wide_width + 40)) points"
elif [ "$wide_status" -ne 0 ] || [ -z "$wide_frame" ]; then
    badmsg "wide window: plain click on primary-button is delivered once  [not attempted: the wide frame could not be computed from list-displays, status=$wide_status]"
else
    "$bin" close-app "$app" --force >/dev/null 2>&1
    launch_fixture "$wide_width" || abort_suite "the fixture could not be relaunched at the wide frame"
    require_target wide_primary button primary-button
    require_value wide_before click-status
    wide_output="$("$bin" click "$(target_ref "$wide_primary")" 2>/dev/null)"
    wide_after="$(await_value click-status click-1)"
    assert "wide window: plain click on primary-button is delivered once" \
        "$([ "$(json_field "$wide_output" ok)" = "True" ] && [ "$wide_before" = "idle" ] && [ "$wide_after" = "click-1" ] && echo 1 || echo 0)" \
        "frame=$wide_frame before=$wide_before after=$wide_after ok=$(json_field "$wide_output" ok) error=$(json_field "$wide_output" error.code) message=$(json_field "$wide_output" error.message)"
    "$bin" close-app "$app" --force >/dev/null 2>&1
    launch_fixture || abort_suite "the fixture could not be relaunched at the normalized frame after the wide-window check"
fi
