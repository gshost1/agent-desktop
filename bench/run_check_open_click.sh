#!/usr/bin/env bash
cd "$(dirname "$0")/.."
bash bench/check_open_click.sh bench/out/wt-base/target/release/agent-desktop \
    bench/out/wt-click/target/release/agent-desktop bench/out/check_open_click.json | tee bench/out/check_open_click.txt
