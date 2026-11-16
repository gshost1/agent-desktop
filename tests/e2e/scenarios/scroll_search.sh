note "find --scroll pages a lazily loaded list until the match loads"
lazy_list=""
require_target lazy_list scrollarea lazy-list
lazy_ref="$(target_ref "$lazy_list")"
"$bin" scroll-to "$lazy_ref" >/dev/null 2>&1

plain_json="$("$bin" find --root "$lazy_ref" --name lazy-row-300 --exact --first 2>/dev/null)"
plain_match="$(json_field "$plain_json" data.match)"
assert "a plain find cannot see a row the lazy list has not loaded" \
    "$([ "$(json_field "$plain_json" ok)" = "True" ] && [ -z "$plain_match" ] && echo 1 || echo 0)" \
    "ok=$(json_field "$plain_json" ok) match=${plain_match:0:80}"

scroll_json="$("$bin" find --root "$lazy_ref" --name lazy-row-300 --exact --first --scroll 2>/dev/null)"
scroll_target="$(printf '%s' "$scroll_json" | "$e2e_python" "$json_tool" target 2>/dev/null)" || scroll_target=""
scroll_pages="$(json_field "$scroll_json" data.scroll.pages)"
assert "find --scroll returns the row and says the view moved" \
    "$([ -n "$scroll_target" ] && [ "$(json_field "$scroll_json" data.scroll.view_moved)" = "True" ] \
        && [ "$(json_field "$scroll_json" data.scroll.stopped)" = "found" ] && [ "${scroll_pages:-0}" -ge 1 ] && echo 1 || echo 0)" \
    "target='$(target_ref "$scroll_target")' scroll=$(json_field "$scroll_json" data.scroll)"

if [ -n "$scroll_target" ]; then
    verify "the ref find --scroll returned acts on that row" lazy-pressed 300 semantic_api "$scroll_target" click
fi

absent_json="$("$bin" find --root "$lazy_ref" --name lazy-row-9999 --exact --first --scroll 2>/dev/null)"
assert "an absent row is a not-found after both ends of the list" \
    "$([ "$(json_field "$absent_json" ok)" = "True" ] && [ -z "$(json_field "$absent_json" data.match)" ] \
        && [ "$(json_field "$absent_json" data.scroll.stopped)" = "both_ends" ] && echo 1 || echo 0)" \
    "scroll=$(json_field "$absent_json" data.scroll) error=$(json_field "$absent_json" error.code)"

"$bin" find --app "$app" --name lazy-row-300 --scroll >/dev/null 2>&1
unrooted_exit=$?
assert "find --scroll without --root is an argument error" \
    "$([ "$unrooted_exit" = "2" ] && echo 1 || echo 0)" "exit=$unrooted_exit"
