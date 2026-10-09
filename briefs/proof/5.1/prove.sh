#!/usr/bin/env bash
# Regenerates the media of briefs/proof/5.1/: a screenshot of each acceptance state of the node
# detail panel and a short video of its main flow. Builds the module (mise run build:wasm) and
# the fixture server, then runs the proof's pictures (web/app/proof/detail.proof.ts), which
# only wait for the state each picture shows. Exits non-zero if the run fails or a picture is missing.
# The README is written by hand.
#
# usage: briefs/proof/5.1/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.1
cd "$repo"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-why-not-relevant 2-due-chain 3-why-blocked 4-gravity-contributors 5-participations
  6a-pin-rejected-with-chain 6b-resolved-by-a-move 7-note-added
  9-artifact-designated-and-completed 10-artifact-removed-stale 11-message-draft 13-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$proof "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/detail.proof.ts --reporter=line)
rm -rf "$proof/video"
missing=0
for picture in "${pictures[@]}"; do
  [ -s "$proof/$picture.png" ] || { echo "prove.sh: the picture $picture is missing" >&2; missing=1; }
done
[ -s "$proof/main-flow.webm" ] || { echo "prove.sh: the main flow's video is missing" >&2; missing=1; }
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
