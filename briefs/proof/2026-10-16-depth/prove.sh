#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-16-depth/: the Depth menu, the graph at Decisions
# hidden and faded, the list at Decisions, the timeline at Deliverables, the walk through at
# Decisions, the graph's View menu on a phone, and a short video of the graph stepping through the
# depths. Builds the module (mise run build:wasm) and runs the proof's pictures
# (web/app/proof/depth.proof.ts) on the in-browser host. Exits non-zero if the run fails or a
# picture is missing. The README is written by hand.
#
# usage: briefs/proof/2026-10-16-depth/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-16-depth
work=$repo/web/app/dist/prove-depth
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

media=(1-depth-menu.png 2-decisions-hidden.png 3-decisions-faded.png 4-list-decisions.png 5-timeline-deliverables.png 6-walk-decisions.png 7-phone-view-menu.png main-flow.webm)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/depth.proof.ts --reporter=line)
missing=0
for each in "${media[@]}"; do
  if [ -s "$work/$each" ]; then cp "$work/$each" "$proof/"; else echo "prove.sh: $each is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
