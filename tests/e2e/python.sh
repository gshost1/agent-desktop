#!/usr/bin/env bash
# Resolves the Python 3.10+ interpreter every E2E script runs its helpers with.
# Order: AGENT_DESKTOP_E2E_PYTHON when it is new enough, then python3.14 down to
# python3.10 on PATH, then python3. The result is exported so child scripts
# reuse it without probing again. An empty result means no suitable interpreter.

e2e_python_candidates=(python3.14 python3.13 python3.12 python3.11 python3.10 python3)

e2e_python_is_suitable() {
    [ -n "$1" ] && "$1" -c 'import sys; sys.exit(sys.version_info < (3, 10))' >/dev/null 2>&1
}

resolve_e2e_python() {
    local name found
    for name in "${e2e_python_candidates[@]}"; do
        found="$(command -v "$name" 2>/dev/null)" || continue
        if e2e_python_is_suitable "$found"; then
            printf '%s' "$found"
            return 0
        fi
    done
    return 1
}

describe_e2e_python_candidates() {
    local name found
    for name in "${e2e_python_candidates[@]}"; do
        found="$(command -v "$name" 2>/dev/null)" || continue
        printf '  %s -> %s (%s)\n' "$name" "$found" "$("$found" --version 2>&1)"
    done
}

require_e2e_python() {
    if [ -n "$e2e_python" ]; then
        return 0
    fi
    {
        echo "the macOS E2E harness needs Python 3.10 or newer and found none."
        echo "It looks for AGENT_DESKTOP_E2E_PYTHON, then ${e2e_python_candidates[*]} on PATH. Found:"
        describe_e2e_python_candidates
        echo "Install Python 3.10 or newer (for example 'brew install python@3.12') or set"
        echo "AGENT_DESKTOP_E2E_PYTHON to its absolute path, then rerun."
    } >&2
    return 1
}

if ! e2e_python_is_suitable "${AGENT_DESKTOP_E2E_PYTHON:-}"; then
    if [ -n "${AGENT_DESKTOP_E2E_PYTHON:-}" ]; then
        echo "AGENT_DESKTOP_E2E_PYTHON=$AGENT_DESKTOP_E2E_PYTHON is not Python 3.10 or newer; ignoring it and searching PATH." >&2
    fi
    AGENT_DESKTOP_E2E_PYTHON="$(resolve_e2e_python)" || AGENT_DESKTOP_E2E_PYTHON=""
fi
export AGENT_DESKTOP_E2E_PYTHON
e2e_python="$AGENT_DESKTOP_E2E_PYTHON"
