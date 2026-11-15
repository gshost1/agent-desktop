#!/usr/bin/env bash
#
# Fails when an error built as delivered_unverified or uncertain tells the
# caller to retry without reading the state first. The rules and their limits
# are in scripts/check_retry_advice.py, which also runs its own self-test.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/scripts/check_retry_advice.py" "$@"
