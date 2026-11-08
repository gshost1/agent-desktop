# Sourced by bench scripts. Every bench build shares one cargo target dir so the
# bench worktrees do not each grow a multi-GB target/ (they filled the disk on
# 2026-11-07). Each tree still gets its own release binary, copied to
# <tree>/target/release/agent-desktop, where driver.py and the probes look.
export CARGO_TARGET_DIR="${BENCH_CARGO_TARGET_DIR:-$HOME/.agent-desktop-bench-target}"
BENCH_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bench_build() {
    local dir="$1"
    (cd "$dir" && bash "$BENCH_ROOT/serial_cargo.sh" cargo build --release -q -p agent-desktop) || return 1
    mkdir -p "$dir/target/release"
    cp "$CARGO_TARGET_DIR/release/agent-desktop" "$dir/target/release/agent-desktop"
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
