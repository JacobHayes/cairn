#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-10-graph-first/: a screenshot of each state of the
# graph-first journey page (the graph with its Frontier strip, the frontier list, the tree list,
# a node's context menu, the walk through, a zoomed-out graph), each in the light and the dark
# theme. Builds the module (mise run build:wasm) and runs the proof's pictures
# (web/app/proof/graph-first.proof.ts). Exits non-zero if the run fails or a picture is missing.
# The README is written by hand.
#
# usage: briefs/proof/2026-10-10-graph-first/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-10-graph-first
work=$repo/web/app/dist/prove-graph-first
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

pictures=(1-graph 2-list-frontier 3-list-tree 4-context-menu 5-walk-through 6-graph-zoomed-out)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/graph-first.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  for theme in light dark; do
    if [ -s "$work/$picture-$theme.png" ]; then cp "$work/$picture-$theme.png" "$proof/"; else echo "prove.sh: the picture $picture-$theme is missing" >&2; missing=1; fi
  done
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
