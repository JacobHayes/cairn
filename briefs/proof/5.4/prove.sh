#!/usr/bin/env bash
# Regenerates the media of briefs/proof/5.4/: a screenshot of each acceptance state of the
# decision view, the timeline, and the status summary, and a short video of their main flow.
# Builds the module (mise run build:wasm) and the fixture server, then runs the proof's
# pictures (web/app/proof/views.proof.ts), which only wait for the state each picture shows. Exits
# non-zero if the run fails or a picture is missing. The README is written by hand.
#
# usage: briefs/proof/5.4/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.4
work=$repo/web/app/dist/prove-5.4
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-decision-view 2-partner-decision-revised 3-timeline-final-anchor 4-timeline-shortfall-and-why
  5-timeline-without-a-final-milestone 6-summary 7-summary-printed)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/views.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else echo "prove.sh: the main flow's video is missing" >&2; missing=1; fi
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
