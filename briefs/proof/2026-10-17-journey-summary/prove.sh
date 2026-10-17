#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-17-journey-summary/: the Summary on the fixtures
# in its distinct states (mid-journey, with trouble, fresh, stalled, completed, archived, empty,
# on a phone, in the dark theme, in print) and a short video of the main flow. Builds the module
# (mise run build:wasm) and runs the proof's pictures (web/app/proof/summary.proof.ts) on the
# in-browser host. Exits non-zero if the run fails or a picture is missing. The README is written
# by hand.
#
# usage: briefs/proof/2026-10-17-journey-summary/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-17-journey-summary
work=$repo/web/app/dist/prove-journey-summary
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm

media=(1-mid-journey.png 2-trouble.png 3-fresh.png 4-stalled.png 5-completed.png 6-archived.png 7-empty.png 8-phone.png 8-phone-full.png 9-dark.png 10-print.png 11-long.png 11-long-phone.png main-flow.webm)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/summary.proof.ts --reporter=line)
missing=0
for each in "${media[@]}"; do
  if [ -s "$work/$each" ]; then cp "$work/$each" "$proof/"; else echo "prove.sh: $each is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof" >&2
