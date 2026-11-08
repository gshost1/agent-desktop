# Sourced by bench scripts. Builds go through serial_cargo.sh, which gives each
# checkout its own target dir under ~/.agent-desktop-bench-target/trees (one dir
# shared across worktrees reused another tree's crates, 2026-11-08). Each tree's
# release binary is copied to <tree>/target/release/agent-desktop, where
# driver.py and the probes look, with its source commit in agent-desktop.commit.
BENCH_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bench_build() {
    local dir="$1" target
    target="$(cd "$dir" && bash "$BENCH_ROOT/serial_cargo.sh" --target-dir)" || return 1
    (cd "$dir" && bash "$BENCH_ROOT/serial_cargo.sh" cargo build --release -q -p agent-desktop) || return 1
    mkdir -p "$dir/target/release"
    cp "$target/release/agent-desktop" "$dir/target/release/agent-desktop"
    (cd "$dir" && git rev-parse HEAD) > "$dir/target/release/agent-desktop.commit"
}

bench_prune_tree() {
    local dir="$1" keep
    [ -d "$dir/target" ] || return 0
    keep=$(mktemp -d "${TMPDIR:-/tmp}/bench-keep.XXXXXX")
    [ -x "$dir/target/release/agent-desktop" ] && cp "$dir/target/release/agent-desktop" "$keep/"
    rm -rf "$dir/target"
    if [ -f "$keep/agent-desktop" ]; then
        mkdir -p "$dir/target/release"
        mv "$keep/agent-desktop" "$dir/target/release/agent-desktop"
    fi
    rmdir "$keep" 2>/dev/null
    return 0
}
