#!/bin/zsh
# `claude` for a pass on this Mac: the pinned Claude Code 2.1.321 binary (the bench Mac's build, sha256 checked by
# final_local.sh) in bare mode on the bench API key, as on the bench Mac: never OAuth, the keychain or the account
# login. The key is read from its owner-only file (HO_API_KEY_FILE) into this process's environment only; it is never
# on a command line or in a log. logwrap.zsh drops it for every tool call. (CLAUDE_CODE_SUBPROCESS_ENV_SCRUB would also
# drop it from the Bash tool, but it forces the default permission mode, which refuses the bench's Bash calls.)
# Updates, telemetry and error reports are off.
#
# With HO_MODEL_STUB_PORT set (a dry run) it talks to local/model_stub.py on 127.0.0.1 instead, with a dummy key, or
# with the bench key when HO_STUB_WITH_KEY=1 (to check which key the pass sends; the stub logs only a hash prefix).
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN ANTHROPIC_BASE_URL CLAUDE_CODE_OAUTH_TOKEN
export DISABLE_AUTOUPDATER=1 DISABLE_TELEMETRY=1 DISABLE_ERROR_REPORTING=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
key_file=${HO_API_KEY_FILE:-}
if [[ -n ${HO_MODEL_STUB_PORT:-} ]]; then
  export ANTHROPIC_BASE_URL=http://127.0.0.1:$HO_MODEL_STUB_PORT
  [[ ${HO_STUB_WITH_KEY:-} == 1 ]] || key_file=""
  export ANTHROPIC_API_KEY=stub-not-a-key
fi
if [[ -n $key_file ]]; then
  [[ -r $key_file && -s $key_file ]] || { print -u2 "bench: API key file missing"; exit 78; }
  export ANTHROPIC_API_KEY="$(<$key_file)"
elif [[ -z ${HO_MODEL_STUB_PORT:-} ]]; then
  print -u2 "bench: no API key file (HO_API_KEY_FILE)"; exit 78
fi
unset HO_API_KEY_FILE
exec "$HOME/.agent-desktop-bench-target/remote/stage/claude-code-2.1.321-bin/claude" --bare "$@"
