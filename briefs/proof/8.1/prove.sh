#!/usr/bin/env bash
# Regenerates the pictures of briefs/proof/8.1/: the detail header of a node an answer ruled
# out, and the canvas chips. Runs web/app/proof/display-state.proof.ts, which asserts what each
# shows. The README is written by hand.
#
# usage: briefs/proof/8.1/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/8.1
cd "$repo"
mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$proof "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/display-state.proof.ts --reporter=line)
for picture in 1-ruled-out-header 2-canvas-chips; do
  [ -s "$proof/$picture.png" ] || { echo "prove.sh: the picture $picture is missing" >&2; exit 1; }
done
echo "prove.sh: wrote the pictures of $proof"
