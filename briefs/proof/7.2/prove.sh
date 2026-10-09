#!/usr/bin/env bash
# Regenerates the media of briefs/proof/7.2/: a screenshot of each state of an answer's
# rationale (given on a triage card, read in the decision view and node detail, a new answer
# starting empty, history keeping each answer's own). Builds the module and the fixture
# server, then runs web/app/proof/rationale.proof.ts, which asserts the state each picture
# shows. The README is written by hand.
#
# usage: briefs/proof/7.2/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/7.2
work=$repo/web/app/dist/prove-7.2
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-answer-with-a-reason 2-decision-view-and-detail 3-a-new-answer-starts-empty 4-history-keeps-each-reason)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/rationale.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]
echo "prove.sh: wrote the media of $proof"
