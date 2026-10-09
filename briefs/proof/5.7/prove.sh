#!/usr/bin/env bash
# Generates briefs/proof/5.7/'s media (the README beside them is written by hand): a screenshot
# of each acceptance state of proposal review (an upgrade to the vendor evaluation's version 2
# with its conflicts resolved and applied; a stale proposal refreshed and reviewed again; the
# routeless journey saved as a route and re-linked; a placeholder broken down from triage) and
# a short video of the upgrade. Builds the module (mise run build:wasm) and the fixture server,
# runs proposal review's web unit tests (Vitest, the offers checked against the module's
# engine) and browser tests (Playwright, e2e/proposals.spec.ts), runs the proof's pictures
# (web/app/proof/proposals.proof.ts), which only wait for the state each picture shows, then checks the planted
# bug: a refresh that carries the reviewer's confirmation over, caught by I6's tests. The plant
# is made in place and always restored (checked byte for byte). Exits non-zero if any outcome
# differs from the one expected.
#
# usage: briefs/proof/5.7/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.7
work=$repo/web/app/dist/prove-5.7
export CARGO_TERM_COLOR=never
mismatches=0
miss() {
  mismatches=$((mismatches + 1))
  echo "prove.sh: $1" >&2
}
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

# Proposal review's tests.
(cd web/app && "$repo/node_modules/.bin/vitest" run src/proposals >/dev/null) || miss "proposal review's web unit tests failed"
(cd web/app && "$repo/node_modules/.bin/playwright" test e2e/proposals.spec.ts --reporter=line >/dev/null) \
  || miss "proposal review's browser tests failed"

# The pictures.
pictures=(1-overview-proposes 2-upgrade-as-a-diff 3-conflicts-resolved 4-applied 5-stale 6-refreshed-review-again
  7-save-as-route-mapped 8-relinked 9-break-down-from-triage 10-breakdown-with-its-frontier 11-dark-theme 12-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/proposals.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi

# The planted bug, in place, restored on any exit: a refresh carries the confirmation over.
model=web/app/src/proposals/model.ts
cp "$model" "$work/model.ts"
restore() { cp "$work/model.ts" "$repo/$model"; }
trap restore EXIT
node -e '
  const fs = require("fs");
  const path = process.argv[1];
  const text = fs.readFileSync(path, "utf8");
  const planted = text.replace("    case \"refreshed\":\n    case \"saved\":\n      return reviewed;", "    case \"refreshed\":\n      return event.proposal.revision;\n    case \"saved\":\n      return reviewed;");
  fs.writeFileSync(path, planted);
' "$model"
cmp -s "$model" "$work/model.ts" && miss "the planted bug was not planted"
status=0
(cd web/app && "$repo/node_modules/.bin/vitest" run src/proposals/model.test.ts >/dev/null 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$model" "$work/model.ts" || miss "model.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"

if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcomes differ from those expected" >&2
  exit 1
fi
echo "prove.sh: wrote $proof"
