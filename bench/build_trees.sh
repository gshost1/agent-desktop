#!/usr/bin/env bash
# Rebuilds the release binaries the queued rounds read directly: this checkout
# (rounds with no tree), wt-base and wt-main at fork main. Builds go through the
# shared bench target dir (bench/cargo_env.sh).
set -u
cd "$(dirname "$0")/.."
source bench/cargo_env.sh
git -C bench/out/wt-main fetch -q origin && git -C bench/out/wt-main checkout -q --detach origin/main
for dir in . bench/out/wt-base bench/out/wt-main; do
    bench_build "$dir" || { echo "build failed in $dir" >&2; exit 1; }
    echo "$dir $(git -C "$dir" rev-parse --short HEAD) built"
done
