#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-17-one-address-per-journey/: Home, the index,
# a journey at its default view, a node link that draws a hidden node, and a short video of
# clicking around with the address shown. Runs web/app/proof/addresses.proof.ts on the
# in-browser host. Exits non-zero if the run fails or a file is missing.
#
# usage: briefs/proof/2026-10-17-one-address-per-journey/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-17-one-address-per-journey
work=$repo/web/app/dist/prove-addresses
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/addresses.proof.ts --reporter=line)
for file in 1-home.png 2-journeys.png 3-journey-summary.png 4-node-link-draws-the-node.png main-flow.webm; do
  [ -s "$work/$file" ] || { echo "prove.sh: $file is missing" >&2; exit 1; }
  cp "$work/$file" "$proof/"
done
echo "prove.sh: wrote the media of $proof" >&2
