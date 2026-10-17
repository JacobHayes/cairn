#!/usr/bin/env bash
# Regenerates the picture of briefs/proof/2026-10-16-landing-without-a-jump/: the screen
# Cairn opens on. Runs the proof's picture
# (web/app/proof/landing.proof.ts), on the in-browser host. Exits non-zero if the run fails or the picture is missing.
#
# usage: briefs/proof/2026-10-16-landing-without-a-jump/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/2026-10-16-landing-without-a-jump
work=$repo/web/app/dist/prove-landing
cd "$repo"
rm -rf "$work"
mkdir -p "$work"


(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/landing.proof.ts --reporter=line)
[ -s "$work/1-opening-cairn.png" ] || { echo "prove.sh: the picture is missing" >&2; exit 1; }
cp "$work/1-opening-cairn.png" "$proof/"
echo "prove.sh: wrote the media of $proof" >&2
