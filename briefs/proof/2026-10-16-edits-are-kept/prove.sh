#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-16-edits-are-kept/: a screenshot of each state of
# a rejected rename (on its screen, away from it, after a reload, brought back, discarded) and of
# a rename that got no answer, and a short video of the first. Runs the proof's pictures
# (web/app/proof/edits.proof.ts) on the server host: it builds the web app and the binary
# first. Exits non-zero if the run fails or a picture is missing. The README is written by hand.
#
# usage: briefs/proof/2026-10-16-edits-are-kept/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-16-edits-are-kept
work=$repo/web/app/dist/prove-edits-are-kept
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:web
cargo test --locked --quiet --workspace --all-features --tests --no-run

pictures=(1-rejected 2-away 3-after-reload 4-back 5-discarded 6-could-not-confirm)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/edits.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else echo "prove.sh: the video is missing" >&2; missing=1; fi
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
