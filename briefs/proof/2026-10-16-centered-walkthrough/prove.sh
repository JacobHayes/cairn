#!/usr/bin/env bash
# Regenerates the "after" media of briefs/proof/2026-10-16-centered-walkthrough/: the
# walkthrough's card at a desktop width and a phone width, and the decision walkthrough with its
# introduction. Runs the proof's pictures (web/app/proof/walkthrough-center.proof.ts) on the
# in-browser host only. The "before" pictures are the same three, taken on the commit before this
# change and kept as they are. Exits non-zero if the run fails or a picture is missing.
# The README is written by hand.
#
# usage: briefs/proof/2026-10-16-centered-walkthrough/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-16-centered-walkthrough
work=$repo/web/app/dist/prove-centered-walkthrough
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

pictures=(1-desktop 2-phone 3-decisions-desktop)
(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/walkthrough-center.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/after-$picture.png"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
