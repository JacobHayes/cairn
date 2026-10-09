#!/usr/bin/env bash
# Regenerates the media of briefs/proof/8.6/: a screenshot of each page and projection of a
# journey, the journey card, the menus, the filter with its chips, the Summary page, and
# the phone and tablet layouts.
# Builds the module (mise run build:wasm) and the fixture server, then runs the proof's
# pictures (web/app/proof/pages.proof.ts), which assert the state each picture shows. Exits
# non-zero if the run fails or a picture is missing. The README is written by hand.
#
# usage: briefs/proof/8.6/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/8.6
work=$repo/web/app/dist/prove-8.6
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-walkthrough-on-start 2-next-list-and-journey-card 3-plan-graph 4-plan-list 5-plan-timeline
  6-decision-view 7-filter-open 8-active-filter-chips 9-journey-menu 10-summary-page
  11-phone-menu 12-tablet-toolbar)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/pages.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
