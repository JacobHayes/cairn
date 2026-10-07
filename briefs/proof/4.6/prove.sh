#!/usr/bin/env bash
# Regenerates the media of briefs/proof/4.6/: a screenshot of each acceptance state of the app
# shell and a short video of its main flow. Builds the module (mise run build:wasm) and the
# fixture server, then runs the proof's pictures (web/app/proof/shell.proof.ts), which assert
# the state each picture shows. Exits non-zero if the run fails or a picture is missing. The
# README is written by hand.
#
# usage: briefs/proof/4.6/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/4.6
cd "$repo"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-index-in-browser 2-journey-derived-in-worker 3a-patch-saved-with-consequences
  3b-other-tab-updated-live 4-conflict-surfaced 5-version-skew-banner 6-draft-survives-reload)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$proof "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/shell.proof.ts --reporter=line)
rm -rf "$proof/video"
missing=0
for picture in "${pictures[@]}"; do
  [ -s "$proof/$picture.png" ] || { echo "prove.sh: the picture $picture is missing" >&2; missing=1; }
done
[ -s "$proof/main-flow.webm" ] || { echo "prove.sh: the main flow's video is missing" >&2; missing=1; }
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
