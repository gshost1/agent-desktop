#!/bin/bash
# Refuses a PR/issue body that could ping upstream: @mentions outside backticks or any vercel-labs text.
# Usage: pr_body_check.sh body.md   (exit 0 = clean)
f=$1
hits=$(sed -E 's/`[^`]*`//g' "$f" | grep -nE '@[A-Za-z0-9-]+|vercel-labs')
if [ -n "$hits" ]; then echo "BLOCKED: $f"; echo "$hits"; exit 1; fi
grep -nE '@[A-Za-z0-9-]+|vercel-labs' "$f" | sed 's/^/  in backticks (ok): /'
echo "clean: $f"
