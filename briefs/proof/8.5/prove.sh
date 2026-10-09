#!/usr/bin/env bash
# Regenerates the media of briefs/proof/8.5/: a picture of the sync chip in each state it can say
# and of a save's receipt (web/app/proof/sync.proof.ts, which asserts the state each picture
# shows). Builds the module (mise run build:wasm) and the fixture server, runs the pictures into
# a scratch directory, and copies them beside the README. Exits non-zero if the run fails or a
# picture is missing. The README is written by hand.
#
# usage: briefs/proof/8.5/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/8.5
work=$repo/web/app/dist/prove-8.5
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(in-sync saved receipt popover saving updating behind reconnecting offline new-version not-saved conflict demo)
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/sync.proof.ts --project=chromium --project=server --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: $picture.png is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
