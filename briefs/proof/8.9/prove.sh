#!/usr/bin/env bash
# Regenerates the media of briefs/proof/8.9/: a screenshot of each state of the Next page (the
# list, its folds, nothing needs you now, the stalled journey), of the cards (a card with the
# pass rail, a node opened from the pass, the end of a pass, the walkthrough on start) and of
# Mine, and a short video of a walkthrough pass.
# Builds the module (mise run build:wasm) and the fixture server, then runs the proof's
# pictures (web/app/proof/next-page.proof.ts), which assert the state each picture shows.
# Exits non-zero if the run fails or a picture is missing. The README is written by hand.
#
# usage: briefs/proof/8.9/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/8.9
work=$repo/web/app/dist/prove-8.9
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-the-next-list 2-the-folds-open 3-nothing-needs-you-now 4-stalled-with-unsnooze 5-a-card-and-the-pass-rail
  6-a-node-opened-from-the-pass 7-every-card-seen-once 8-the-walkthrough-on-start 9-mine)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/next-page.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
if [ -s "$work/a-walkthrough-pass.webm" ]; then cp "$work/a-walkthrough-pass.webm" "$proof/"; else echo "prove.sh: the video is missing" >&2; missing=1; fi
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
