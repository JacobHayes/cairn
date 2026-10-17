#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-16-view-picker/: the journey header on the graph,
# the View picker open, the `⋯` menu with Status open, an archived journey with its chip, the
# same on a phone with the picker closed and open, and a short video of the picker taking the
# page through every view. Builds the module (mise run build:wasm) and runs the proof's pictures
# (web/app/proof/view-header.proof.ts) on the in-browser host. Exits non-zero if the run fails or
# a picture is missing. The README is written by hand.
#
# usage: briefs/proof/2026-10-16-view-picker/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-16-view-picker
work=$repo/web/app/dist/prove-view-picker
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

media=(1-header.png 2-view-picker.png 3-status-menu.png 4-archived.png 5-phone-archived.png 6-phone-picker.png main-flow.webm)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/view-header.proof.ts --reporter=line)
missing=0
for each in "${media[@]}"; do
  if [ -s "$work/$each" ]; then cp "$work/$each" "$proof/"; else echo "prove.sh: $each is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
