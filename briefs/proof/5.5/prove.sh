#!/usr/bin/env bash
# Generates briefs/proof/5.5/: the README, a screenshot of each acceptance state of the journey
# index and overview, the cross-journey "mine" list, journey creation, the route screens,
# entities, and identity, and a short video of the main flow, each step checked as it is
# taken. Builds the module (mise run build:wasm) and the fixture server, runs the screens' web
# unit tests (Vitest) and browser tests (Playwright, e2e/around.spec.ts and
# e2e/around-server.spec.ts in rung 6's web/app suite), runs the proof's pictures
# (web/app/proof/around.proof.ts), then the planted bug: "upgrade available" hidden on a
# retired route, caught by C17's retired-route unit test. The plant is made in place and
# always restored (checked byte for byte). Exits non-zero if any outcome differs from the one
# expected.
#
# usage: briefs/proof/5.5/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.5
work=$repo/web/app/dist/prove-5.5
readme=$proof/README.md
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

# The screens' web unit tests.
unit=$work/vitest.json
(cd web/app && "$repo/node_modules/.bin/vitest" run src/journeys src/routes src/people src/data/live.test.ts src/data/server-host.test.ts src/detail/AnswerEditor.test.ts \
  --reporter=json --outputFile.json="$unit" >/dev/null) || miss "the screens' web unit tests failed"
# The screens' browser tests.
e2e=$work/playwright.json
(cd web/app && PLAYWRIGHT_JSON_OUTPUT_NAME=$e2e "$repo/node_modules/.bin/playwright" test e2e/around.spec.ts e2e/around-server.spec.ts --reporter=json >/dev/null) \
  || miss "the screens' browser tests failed"
# The pictures, each step checked as it was taken, and the values they showed.
pictures=(1-route-index 2-route-detail-three-versions-retired 3-index-filtered-upgrade-available 4-imported-as-a-draft
  5-import-refused-while-a-draft-is-open 6-journey-index 7-mine-across-journeys 8-new-journey-from-a-route
  9-lands-in-the-walkthrough 10-overview 11-archived-with-delete-behind-its-name 12-entities-merged
  13-the-journey-reads-the-survivor 14-identity-in-browser 15-identity-linked-on-the-server 16-dark-theme 17-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/around.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi
node -e '
  const values = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const expect = (ok, what) => { if (!ok) { console.error(what); process.exitCode = 1; } };
  const marks = values.routeDetail;
  expect(Object.values(marks["1"]).every((mark) => mark === "available"), "a version 1 journey of the retired route lost its upgrade");
  expect(Object.values(marks["2"]).every((mark) => mark === "available"), "a version 2 journey of the retired route lost its upgrade");
  expect(Object.values(marks["3"]).every((mark) => mark === "none"), "a journey on the latest version was offered an upgrade");
  expect(values.export.identical, "the imported draft did not export identically to version 1");
  expect(values.merge.historyBefore > 0 && values.merge.historyAfter === values.merge.historyBefore, "the merge changed the history shown");
  expect(values.linked.providers.includes("stub"), "the stub identity was not linked");
' "$work/around.json" || miss "the proof's values are not the ones expected"

# The planted bug, in place, restored on any exit: "upgrade available" hidden on a retired route.
model=web/app/src/routes/model.ts
cp "$model" "$work/model.ts"
restore() { cp "$work/model.ts" "$repo/$model"; }
trap restore EXIT
sed -i 's|upgrade: summary?.upgrade_available ?? false };|upgrade: (summary?.upgrade_available ?? false) \&\& read.detail.header.retired !== true };|' "$model"
cmp -s "$model" "$work/model.ts" && miss "the planted bug was not planted"
status=0
planted=$(cd web/app && "$repo/node_modules/.bin/vitest" run src/routes/model.test.ts 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$model" "$work/model.ts" || miss "model.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"
grep -qE '(FAIL|AssertionError)' <<<"$planted" || miss "the planted bug's failure was not found"

out=$readme.tmp
cat >"$out" <<'EOF'
# Proof for brief 5.5: Journey index and overview, route screens, entities, identity

## What you can do now

A header row reaches the screens across journeys: Journeys, Mine, Routes, Entities, and You.

- **Journeys** (C16): the journey index, filtered by status, lineage route and version,
  "mine", and "upgrade available" (each journey's own field from the host). A new journey
  starts from a route's version, the latest by default, or empty, and opens on its decision
  walkthrough (B1). Each journey has an overview tab: its name and description (editable),
  status, lineage with any upgrade, its own notes and links (G1), and its life: complete
  (suggested when nothing in scope is left), reopen, archive, un-archive, and, once
  archived, hard delete behind its name typed back (B11, A19).
- **Mine**: everything that is yours in each active journey, journey by journey, not ranked
  across them (C16, E4).
- **Routes** (C17): every route, and each route's versions with the journeys on each and
  which have an upgrade available, still so once the route is retired (A19). Open a draft,
  publish or discard it (A11), retire the route or bring it back, and export a version or
  the draft as the YAML kept on disk, or import a file as a new route or draft (A13).
- **Entities** (E6, B3): add one with a name alone, add its emails later, and merge two; the
  merged key stays an alias, so journeys and their history read the survivor. An entity
  answer can also name a new entity, made in the same patch.
- **You** (H3): the identities you sign in with and the emails each verified, the entities
  those emails name, the offer to merge them when they are duplicates, and, on the server,
  a sign-in with another provider that links it to you.

Every screen stays current as other pages and people change what it shows (H6). The
pictures are on the in-browser host, seeded from the fixtures, except the linked identity,
on the server host with the auth tests' stub issuer.

## Routes

The route index.

![The route index](1-route-index.png)

The vendor evaluation with versions 2 and 3 published, a journey started on each, then
retired: hidden from new journeys, while the journeys on versions 1 and 2 still show an
upgrade.

![Route detail](2-route-detail-three-versions-retired.png)

The journey index filtered to that route's journeys with an upgrade available.

![Filtered](3-index-filtered-upgrade-available.png)

The hiring loop's fixture file imported as a new draft. Its export is version 1's file,
byte for byte.

![Imported](4-imported-as-a-draft.png)

Importing again while that draft is open is refused, with the option to discard it.

![Refused](5-import-refused-while-a-draft-is-open.png)

## Journeys

The journey index.

![The journey index](6-journey-index.png)

Mine across journeys, for the in-browser host's local user.

![Mine across journeys](7-mine-across-journeys.png)

A new journey from the hiring loop, and the walkthrough it lands in.

![A new journey from a route](8-new-journey-from-a-route.png)

![It lands in the walkthrough](9-lands-in-the-walkthrough.png)

The vendor evaluation's overview.

![The overview](10-overview.png)

Completed, then archived: it accepts only un-archiving or deletion, which waits for its
name.

![Archived](11-archived-with-delete-behind-its-name.png)

## Entities and identity

A new entity kept and the evaluation lead merged into it: the old key is its alias, and its
email moved with it.

![Merged](12-entities-merged.png)

The journey now reads the survivor as owner; the decision that named the lead keeps its
history.

![The survivor](13-the-journey-reads-the-survivor.png)

The local user's verified emails name each fixture's lead, so they are offered for merging.

![Identity in the browser](14-identity-in-browser.png)

On the server, an identity signed in with the stub issuer is linked to the user, and the
entity holding its verified email is now them.

![Linked](15-identity-linked-on-the-server.png)

## Dark theme and a narrow screen

![Dark theme](16-dark-theme.png)

![Narrow screen](17-narrow-screen.png)

The main flow, from a journey started from a route through its walkthrough, its overview
(completed, then archived), and the route's versions: [main-flow.webm](main-flow.webm).

## Known limits

- Upgrade, save-as-route, and re-link are not yet offered from the overview; they come with
  proposal review.
- "Mine" derives each active journey that refers to you in the tab, which costs one derive
  per such journey (`decisions/2026-10-07-mine-across-journeys-is-each-active-journeys-own-mine-found.md`).
- A filtered index refetches once for each new revision of a journey it does not show,
  since that journey may now match.
- The in-browser host's one local user holds every fixture lead's email, so it is always
  offered a merge of them.

Generated by `briefs/proof/5.5/prove.sh`, which also runs the screens' tests.
EOF
mv "$out" "$readme"
if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcomes differ from those expected" >&2
  exit 1
fi
echo "prove.sh: wrote $readme"
