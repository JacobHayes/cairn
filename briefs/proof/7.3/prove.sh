#!/usr/bin/env bash
# Regenerates the media of briefs/proof/7.3/: a screenshot of each state of a node that
# requires a note (done asking for it inline, done with it, stale once it is removed). Builds
# the module and the fixture server, then runs web/app/proof/requires-note.proof.ts, which
# asserts the state each picture shows. The README is written by hand.
#
# usage: briefs/proof/7.3/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/7.3
work=$repo/web/app/dist/prove-7.3
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-done-asks-for-the-note 2-done-with-its-note 3-stale-without-the-note)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/requires-note.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
