#!/usr/bin/env bash
# Generates briefs/proof/5.6/'s media (the README beside them is written by hand): a screenshot of each acceptance state of
# authoring (a route by hand from an empty draft, published, exported, and imported back; a
# fixture journey's structure edited, reset, removed with its cascade, and restored), and a
# short video of the main flow. Builds the module (mise run build:wasm) and the fixture server,
# runs authoring's web unit tests (Vitest, against the module's engine) and browser tests
# (Playwright, e2e/authoring.spec.ts), runs the proof's pictures
# (web/app/proof/authoring.proof.ts), which only wait for the state each picture shows, then checks the planted
# bug: a removal sent without its cascade, caught by A18's cascade tests. The plant is made in
# place and always restored (checked byte for byte). Exits non-zero if any outcome differs from
# the one expected.
#
# usage: briefs/proof/5.6/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.6
work=$repo/web/app/dist/prove-5.6
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

# Authoring's tests.
(cd web/app && "$repo/node_modules/.bin/vitest" run src/authoring >/dev/null) || miss "authoring's web unit tests failed"
(cd web/app && "$repo/node_modules/.bin/playwright" test e2e/authoring.spec.ts --reporter=line >/dev/null) \
  || miss "authoring's browser tests failed"

# The pictures, and the values behind them.
pictures=(2-empty-draft 3-a-decision-fills-a-role 4-a-condition 5-a-date-rule 6-an-edge-to-its-own-stage-refused
  7-the-route-on-its-canvas 8-published-exported-and-imported-back 9-three-violations-at-three-fields
  10-journey-edit-mode-a-local-node 11-a-route-copied-title-edited-here 12-a-removal-and-its-cascade
  14-restored-as-a-local-copy 15-broken-down-by-hand 16-dark-theme 17-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/authoring.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi
node -e '
  const values = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const expect = (ok, what) => { if (!ok) { console.error(what); process.exitCode = 1; } };
  expect(values.export.identical, "the exported route did not import back to the same file");
  expect(JSON.stringify(values.violations.fields) === JSON.stringify(["id", "opens_at", "closes_at"]), "the three violations were not at three fields");
  expect(values.journey.rewrites.some(([node, what]) => node === "n_baseline" && what === "condition"), "the removal did not rewrite the dangling condition");
' "$work/authoring.json" || miss "the proof's values are not the ones expected"

# The planted bug, in place, restored on any exit: a removal sent without its cascade.
cascade=web/app/src/authoring/cascade.ts
cp "$cascade" "$work/cascade.ts"
restore() { cp "$work/cascade.ts" "$repo/$cascade"; }
trap restore EXIT
sed -i 's|  return \[...plan.dangling.map((each) => each.mutation), { op: "remove_node", removal: plan.removal }\];|  return [{ op: "remove_node", removal: plan.removal }];|' "$cascade"
cmp -s "$cascade" "$work/cascade.ts" && miss "the planted bug was not planted"
status=0
(cd web/app && "$repo/node_modules/.bin/vitest" run src/authoring/cascade.test.ts >/dev/null 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$cascade" "$work/cascade.ts" || miss "cascade.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"

if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcomes differ from those expected" >&2
  exit 1
fi
echo "prove.sh: wrote $proof"
