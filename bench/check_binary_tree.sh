#!/usr/bin/env bash
# Fails unless <tree>/target/release/agent-desktop was built from <tree>'s own
# sources. The release binary embeds absolute source paths of agent-desktop-core;
# every one must sit under <tree>. A shared cargo target dir once linked one
# worktree's core into another's binary (2026-11-08).
set -u
tree="$(cd "${1:?usage: check_binary_tree.sh <tree>}" && pwd -P)"
bin="$tree/target/release/agent-desktop"
[ -x "$bin" ] || { echo "check_binary_tree: no binary at $bin" >&2; exit 1; }
paths=$(strings -n 8 "$bin" | grep -o '/Users/[^ ]*/crates/[a-z_]*/' | sort -u)
[ -n "$paths" ] || { echo "check_binary_tree: no embedded source paths in $bin" >&2; exit 1; }
foreign=$(printf '%s\n' "$paths" | grep -v "^$tree/crates/")
if [ -n "$foreign" ]; then
    echo "check_binary_tree: $bin holds crates from another tree: $foreign" >&2
    exit 1
fi
echo "check_binary_tree: $bin built from $tree"
