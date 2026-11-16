#!/bin/zsh
# `claude` for a pass on this Mac: the pinned Claude Code 2.1.321 binary (the bench Mac's build, sha256 checked by
# final_local.sh), on the logged-in account, with updates, telemetry and error reports off. No API key, auth token
# or base URL from the caller reaches it. With HO_MODEL_STUB_PORT set (a dry run) it talks to local/model_stub.py
# with a dummy key instead, and no request leaves the Mac.
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN ANTHROPIC_BASE_URL CLAUDE_CODE_OAUTH_TOKEN
export DISABLE_AUTOUPDATER=1 DISABLE_TELEMETRY=1 DISABLE_ERROR_REPORTING=1 CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1
if [[ -n ${HO_MODEL_STUB_PORT:-} ]]; then
  export ANTHROPIC_API_KEY=stub-not-a-key ANTHROPIC_BASE_URL=http://127.0.0.1:$HO_MODEL_STUB_PORT
fi
exec "$HOME/.agent-desktop-bench-target/remote/stage/claude-code-2.1.321-bin/claude" "$@"
