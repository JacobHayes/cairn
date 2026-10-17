#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-17-home-and-indexes/: Home, the Journeys index and
# the Routes index on the fixtures (desktop, dark, phone) and a short video of the main flow. Runs
# web/app/proof/home-and-indexes.proof.ts on the in-browser host. The README is written by hand.
#
# usage: briefs/proof/2026-10-17-home-and-indexes/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-17-home-and-indexes
work=$repo/web/app/dist/prove-home-and-indexes
cd "$repo"
rm -rf "$work"
mkdir -p "$work"
mise run build:wasm
media=(1-home.png 2-home-dark.png 3-journeys.png 4-journeys-dark.png 5-routes.png 6-home-phone.png 7-journeys-phone.png 8-routes-phone.png main-flow.webm)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/home-and-indexes.proof.ts --reporter=line)
missing=0
for each in "${media[@]}"; do
  if [ -s "$work/$each" ]; then cp "$work/$each" "$proof/"; else echo "prove.sh: $each is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
