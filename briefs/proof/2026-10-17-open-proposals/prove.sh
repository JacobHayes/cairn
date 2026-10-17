#!/usr/bin/env bash
# Regenerates the media of briefs/proof/2026-10-17-open-proposals/: a journey's Proposals tab with
# a drafted upgrade and empty, Home's panel, the Routes index's Proposed panel, the review of a
# proposal that creates a route, the phone's tab and review, and a short video of finding an open
# proposal from the journey and from Home. Runs web/app/proof/open-proposals.proof.ts on the
# in-browser host. Exits non-zero if the run fails or a file is missing.
#
# usage: briefs/proof/2026-10-17-open-proposals/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-17-open-proposals
work=$repo/web/app/dist/prove-open-proposals
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

(cd web/app && CAIRN_E2E_BINARY=0 CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/open-proposals.proof.ts --reporter=line)
for file in 1-journey-proposals-tab.png 2-no-open-proposals.png 3-home-proposals-to-review.png 4-routes-proposed.png 5-review-of-a-new-route.png 6-phone-journey-proposals.png 7-phone-review.png main-flow.webm; do
  [ -s "$work/$file" ] || { echo "prove.sh: $file is missing" >&2; exit 1; }
  cp "$work/$file" "$proof/"
done
echo "prove.sh: wrote the media of $proof" >&2
