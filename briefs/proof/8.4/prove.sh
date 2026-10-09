#!/usr/bin/env bash
# Regenerates the "after" media of briefs/proof/8.4/: a screenshot of each state of the app frame
# (web/app/proof/frame.proof.ts, which asserts the state each picture shows). Builds the module
# (mise run build:wasm) and the fixture server, runs the pictures into a scratch directory, and
# copies them beside the README. The before-*.png pictures were taken from the frame before the
# migration with the same script, and are kept as they are. Exits non-zero if the run fails or a
# picture is missing. The README is written by hand.
#
# usage: briefs/proof/8.4/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/8.4
work=$repo/web/app/dist/prove-8.4
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(canvas-light canvas-dark list-light list-dark decisions-graph inspector-history tablet-sheet phone-page phone-map phone-sheet you-dark)
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/frame.proof.ts --project=chromium --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: $picture.png is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
