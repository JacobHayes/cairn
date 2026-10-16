#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-16-one-view-state/: the same view state on the
# graph, the list, the timeline and the walk through, the timeline's Filter, the graph's View
# menu on a phone, the empty graph with Reset the view, and a short video of the settings
# following the viewer from view to view. Builds the module (mise run build:wasm) and runs the
# proof's pictures (web/app/proof/view-state.proof.ts) on the in-browser host. Exits non-zero if
# the run fails or a picture is missing. The README is written by hand.
#
# usage: briefs/proof/2026-10-16-one-view-state/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-16-one-view-state
work=$repo/web/app/dist/prove-one-view-state
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

media=(1-graph.png 2-list.png 3-timeline.png 4-walk-through.png 5-timeline-filter.png 6-phone-view-menu.png 7-nothing-matches.png main-flow.webm)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/view-state.proof.ts --reporter=line)
missing=0
for each in "${media[@]}"; do
  if [ -s "$work/$each" ]; then cp "$work/$each" "$proof/"; else echo "prove.sh: $each is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
