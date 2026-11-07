#!/usr/bin/env bash
# The live click half of bench/ab_surface.sh, on its own.
set -u
cd "$(dirname "$0")/.."
bash bench/ab_click.sh bench/out/wt-base/target/release/agent-desktop \
    bench/out/wt-surface/target/release/agent-desktop bench/out/ab_surface.json | tee bench/out/ab_surface_click.txt
