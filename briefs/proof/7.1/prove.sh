#!/usr/bin/env bash
# Regenerates the pictures of briefs/proof/7.1/: the walkthrough after the partner decision is
# answered (what it unlocked comes next, labeled), the same pass with every kind, and the pass
# after a node completed in its inspector. Builds the module (mise run build:wasm), then runs
# web/app/proof/unlocked-first.proof.ts. Exits non-zero if the run fails or a picture is missing.
# The README is written by hand.
#
# usage: briefs/proof/7.1/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/7.1
work=$repo/web/app/dist/prove-7.1
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

pictures=(1-the-follow-up-comes-next 2-every-kind-the-same-pass 3-completed-from-the-pass)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/unlocked-first.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
