#!/usr/bin/env bash
# Regenerates the media of briefs/proof/7.5/: a screenshot of each state of notices
# (web/app/proof/notices.proof.ts, which asserts what each shows). Builds the module
# (mise run build:wasm) and the fixture server first. The README is written by hand.
#
# usage: briefs/proof/7.5/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/7.5
work=$repo/web/app/dist/prove-7.5
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-the-draft-lists-its-notice 2-the-edge-clears-it 3-an-import-lists-its-notices)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/notices.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
