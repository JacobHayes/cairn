#!/usr/bin/env bash
# Regenerates the media of briefs/proof/5.2/: a screenshot of each acceptance state of the
# canvas and a short video of its main flow. Builds the module (mise run build:wasm) and the
# fixture server, then runs the proof's pictures (web/app/proof/canvas.proof.ts), which only wait for
# the state each picture shows, into a scratch directory, and copies the media beside the
# README. Exits non-zero if the run fails or a picture is missing. The README is written by
# hand.
#
# usage: briefs/proof/5.2/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.2
work=$repo/web/app/dist/prove-5.2
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-whole-journey 2-actions-hidden-as-checklists 3-groups-only 4-groups-and-milestones-hidden-marker
  5-drilled-into-setup 6-trace-of-the-test-plan 7-heat-overlay 8a-undecided-ghosted
  8b-decisions-hidden-marker 8c-marker-opened-the-trace 9-stalled-surface 10-route-canvas)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/canvas.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}" main-flow; do
  file=$work/$picture.png
  [ "$picture" = main-flow ] && file=$work/main-flow.webm
  if [ -s "$file" ]; then cp "$file" "$proof/"; else echo "prove.sh: $(basename "$file") is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
