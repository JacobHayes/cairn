#!/usr/bin/env bash
# Generates briefs/proof/5.2/: the README, a screenshot of each acceptance state of the canvas,
# a short video of its main flow, and the layout's moves when a node is added to the fixture.
# Builds the module (mise run build:wasm) and the fixture server, runs the canvas's web unit
# tests (Vitest) and its browser tests (Playwright, e2e/canvas.spec.ts in rung 6's web/app
# suite) keeping their counts, runs the proof's pictures (web/app/proof/canvas.proof.ts), then
# the planted bug: implicit edges drawn solid, caught by C1's unit test. The plant is made in
# place and always restored (checked byte for byte). Exits non-zero if any outcome differs
# from the one expected.
#
# usage: briefs/proof/5.2/prove.sh
# shellcheck disable=SC2016 # backticks in single quotes are Markdown code spans
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.2
work=$repo/web/app/dist/prove-5.2
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

# The canvas's web unit tests: each file and its count.
unit=$work/vitest.json
(cd web/app && "$repo/node_modules/.bin/vitest" run src/canvas --reporter=json --outputFile.json="$unit" >/dev/null) \
  || miss "the canvas's web unit tests failed"
unit_rows=$(node -e '
  const report = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  for (const file of report.testResults) {
    const name = file.name.replace(/.*\/web\/app\//, "");
    const passed = file.assertionResults.filter((test) => test.status === "passed").length;
    console.log(`| \`${name}\` | ${passed} passed, ${file.assertionResults.length - passed} failed |`);
  }' "$unit")

# The canvas's browser tests: each test and its outcome.
e2e=$work/playwright.json
(cd web/app && PLAYWRIGHT_JSON_OUTPUT_NAME=$e2e "$repo/node_modules/.bin/playwright" test e2e/canvas.spec.ts --reporter=json >/dev/null) \
  || miss "the canvas's browser tests failed"
e2e_rows=$(node -e '
  const report = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const walk = (suite) => [
    ...(suite.specs ?? []).map((spec) => `| ${spec.title} | ${spec.ok ? "passed" : "failed"} |`),
    ...(suite.suites ?? []).flatMap(walk),
  ];
  for (const suite of report.suites) for (const row of walk(suite)) console.log(row);
' "$e2e")

# The pictures, and the layout's moves.
pictures=(1-whole-journey 2-actions-hidden-as-checklists 3-groups-only 4-groups-and-milestones-hidden-marker
  5-drilled-into-setup 6-trace-of-the-test-plan 7-heat-overlay 8a-undecided-ghosted
  8b-decisions-hidden-marker 8c-marker-opened-the-trace 9-stalled-surface 10-route-canvas 11-dark-theme
  12-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/canvas.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi
layout=$(node -e '
  const layout = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const moved = layout.moved.length ? layout.moved.map((key) => `\`${key}\``).join(", ") : "none";
  console.log(`${layout.same ? "identical" : "DIFFERENT"}|${layout.moved.length}|${layout.nodes}|${moved}`);' "$work/layout.json") \
  || miss "the layout's moves were not written"
IFS='|' read -r same moved_count node_count moved_list <<<"$layout"
[ "$same" = identical ] || miss "the same graph laid out differently twice"
node -e 'process.exit(Number(process.argv[1]) / Number(process.argv[2]) < 0.25 ? 0 : 1)' "$moved_count" "$node_count" \
  || miss "adding a node moved $moved_count of $node_count nodes"

# The planted bug, in place, restored on any exit: implicit edges drawn solid.
look=web/app/src/canvas/look.ts
cp "$look" "$work/look.ts"
restore() { cp "$work/look.ts" "$repo/$look"; }
trap restore EXIT
sed -i 's|  return { dash: line.implicit ? DOTTED : undefined, classes: classes.filter(Boolean) };|  return { dash: undefined, classes: classes.filter(Boolean) };|' "$look"
cmp -s "$look" "$work/look.ts" && miss "the planted bug was not planted"
status=0
planted=$(cd web/app && "$repo/node_modules/.bin/vitest" run src/canvas/model.test.ts 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$look" "$work/look.ts" || miss "look.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"
caught=$(grep -E '(FAIL|AssertionError)' <<<"$planted" | sed 's/^ *//' | head -n 2 | paste -sd ' ' -)
[ -n "$caught" ] || miss "the planted bug's failure was not found"

out=$readme.tmp
{
  cat <<'EOF'
# Proof for brief 5.2: Canvas, semantic zoom, trace, layout

Generated by `briefs/proof/5.2/prove.sh`; do not edit by hand. The script fails if any
outcome below differs from the one expected.

A journey is now a pannable, zoomable flowchart (C1, C3) in place of 5.1's node list. Each
card shows its kind, title, state, owner (or "unassigned"), and due date; a decision shows its
prompt and answer. Explicit `requires` edges are solid and implicit gates (conditions, stage
openings) dotted, each labeled with its source. Not-relevant nodes are grayed and undecided
ones ghosted, each with a toggle that hides them; each kind shows or hides on its own, with
what is hidden rolled up into its nearest visible container by the engine's level (C2): a
hidden action is a checklist item on its deliverable (C4), and work whose prerequisite has no
visible stand-in carries a "hidden prerequisites" marker that opens the trace. A container
opens as its own canvas and the breadcrumb leads back out (C4). The frontier and active work
are outlined, the viewer's own items marked, and the top-ranked few carry numbered badges; a
journey with nothing to act on shows what it waits on (C5). Due dates carry a quiet urgency
color, latest start and slack sit beside them, gravity weighs each border, and a heat toggle
shows gravity and leverage as numbers (C6). Tracing a node marks its upstream and downstream
across levels, with its gravity contributors, on the canvas as an overlay (C7). ELK lays it out
in a worker, left to right, deterministically, with the previous positions as hints (C15). A
route's graph has its own canvas, with no journey state. A card opens 5.1's node detail beside
the canvas.

All pictures are on the in-browser host, seeded from the fixtures on each load.

## The whole journey and semantic zoom (C1 to C5)

The vendor evaluation with every kind shown: the decision meeting and the final review's
opening are the frontier, ranked 1 and 2; Partner-led is not relevant and grayed; the kickoff's
stage opening and the two conditions are dotted:

![The whole journey](1-whole-journey.png)

Actions hidden: the test plan's two actions are its checklist:

![Actions hidden](2-actions-hidden-as-checklists.png)

Groups only: everything else rolls up into its group, and the edges into the final review
collapse into one:

![Groups only](3-groups-only.png)

Groups and milestones hidden: the final report waits on the final review's opening, which has
no visible stand-in, so it carries the hidden-prerequisites marker:

![The marker](4-groups-and-milestones-hidden-marker.png)

Drilled into Setup, with the breadcrumb back out:

![Drilled in](5-drilled-into-setup.png)

## The trace and the heat overlay (C6, C7)

The test plan traced: its upstream (environment access, its actions, the kickoff), its
downstream through Testing and Reporting, and the two nodes still adding to its gravity:

![The trace](6-trace-of-the-test-plan.png)

The hiring loop with the heat overlay:

![Heat](7-heat-overlay.png)

## Undecided work and hidden decisions (C1, C2)

The hiring loop's offer decision reopened: the offer letter and the close-out are undecided,
ghosted:

![Undecided](8a-undecided-ghosted.png)

Decisions hidden: both carry the marker for the decision they wait on:

![Hidden decisions](8b-decisions-hidden-marker.png)

The marker opens the trace, which names the hidden decision:

![The marker's trace](8c-marker-opened-the-trace.png)

## The stalled surface (C5, D5)

The hiring loop's only acting item snoozed: nothing can be acted on, and the surface says what
the journey waits on:

![Stalled](9-stalled-surface.png)

## A route's canvas, a dark theme, a narrow screen

The vendor evaluation's route, version 1, with no journey state:

![A route's canvas](10-route-canvas.png)

![Dark theme](11-dark-theme.png)

![Narrow screen](12-narrow-screen.png)

The main flow: kinds hidden and shown again, Setup drilled into and back out, the test plan
opened and traced: [main-flow.webm](main-flow.webm).

## The layout (C15)

EOF
  echo "Two pages laying out the vendor evaluation: $same positions for all $node_count nodes."
  echo
  echo "One action added under Reporting, requiring the findings (on the server host, the view's"
  echo "previous positions as hints): $moved_count of $node_count nodes moved within their containers"
  echo "($moved_list), under the stated bound of a quarter ([\`decisions/2026-10-07-the-layout-is-hinted-by-the-views-last-positions-a-fresh.md\`](../../../decisions/2026-10-07-the-layout-is-hinted-by-the-views-last-positions-a-fresh.md))."
  cat <<'EOF'

## The tests

The canvas's web unit tests (Vitest, rung 6):

| File | Result |
|---|---|
EOF
  printf '%s\n' "$unit_rows"
  cat <<'EOF'

The canvas's browser tests (Playwright in Chromium, rung 6, `web/app/e2e/canvas.spec.ts`):

| Test | Result |
|---|---|
EOF
  printf '%s\n' "$e2e_rows"
  cat <<'EOF'

## The planted bug

Implicit edges drawn solid (`look.ts` drops the dotted dash): C1's unit tests for the
condition gate and the stage opening fail:

```text
EOF
  printf '%s\n' "$caught"
  echo '```'
} >"$out"
mv "$out" "$readme"
if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcomes differ from those expected" >&2
  exit 1
fi
echo "prove.sh: wrote $readme"
