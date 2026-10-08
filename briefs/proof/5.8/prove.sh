#!/usr/bin/env bash
# Generates briefs/proof/5.8/'s media (the README beside them is written by hand): a screenshot
# of each acceptance state of the assistant panel on the server host against the scripted
# model (the panel opened, a turn in progress, a direct change reported with its node, a
# breakdown drafted as a proposal, that proposal reviewed and applied, the conversation read
# back on the overview, a route's draft, the in-browser host without the panel, dark and
# narrow) and a short video of the main flow. Builds the module (mise run build:wasm) and the
# fixture server, runs the panel's web unit tests (Vitest) and browser tests (Playwright,
# e2e/assistant.spec.ts), runs the proof's pictures (web/app/proof/assistant.proof.ts), which
# assert what each shows, then checks the planted bug: the panel offered without the
# capability, caught by the gating test. The plant is made in place and always restored
# (checked byte for byte). Exits non-zero if any outcome differs from the one expected.
#
# usage: briefs/proof/5.8/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.8
work=$repo/web/app/dist/prove-5.8
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

# The panel's tests.
(cd web/app && "$repo/node_modules/.bin/vitest" run src/assistant >/dev/null) || miss "the panel's web unit tests failed"
(cd web/app && "$repo/node_modules/.bin/playwright" test e2e/assistant.spec.ts --reporter=line >/dev/null) \
  || miss "the panel's browser tests failed"

# The pictures, each asserted as it is taken.
pictures=(1-panel-opened 2-working 3-direct-change-reported 4-breakdown-proposed 5-proposal-in-review
  6-applied-on-the-canvas 7-overview-reads-it-back 8-route-draft 9-in-browser-host-without-it 10-dark-theme
  11-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/assistant.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi

# The planted bug, in place, restored on any exit: the panel offered without the capability.
model=web/app/src/assistant/model.ts
cp "$model" "$work/model.ts"
restore() { cp "$work/model.ts" "$repo/$model"; }
trap restore EXIT
node -e '
  const fs = require("fs");
  const path = process.argv[1];
  const text = fs.readFileSync(path, "utf8");
  const planted = text.replace("return session.capabilities.assistant && session.host.assistant !== undefined;", "return session.host.assistant !== undefined;");
  fs.writeFileSync(path, planted);
' "$model"
cmp -s "$model" "$work/model.ts" && miss "the planted bug was not planted"
status=0
(cd web/app && "$repo/node_modules/.bin/vitest" run src/assistant/AssistantPanel.test.tsx >/dev/null 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$model" "$work/model.ts" || miss "model.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"

if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcomes differ from those expected" >&2
  exit 1
fi
echo "prove.sh: wrote $proof"
