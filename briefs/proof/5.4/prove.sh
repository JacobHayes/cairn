#!/usr/bin/env bash
# Generates briefs/proof/5.4/: the README, a screenshot of each acceptance state of the decision
# view, the timeline, and the status summary, and a short video of their main flow. Builds the
# module (mise run build:wasm) and the fixture server, runs the three screens' web unit tests
# (Vitest) and browser tests (Playwright, e2e/decisions.spec.ts, e2e/timeline.spec.ts, and
# e2e/summary.spec.ts in rung 6's web/app suite), runs the proof's pictures
# (web/app/proof/views.proof.ts), then checks the planted bug (the brief's decision log
# records it): the timeline anchored on the latest pin instead of the final milestone, caught
# by C13's unit tests. The plant is made in place and always restored (checked byte for byte).
# Exits non-zero if any outcome differs from the one expected.
#
# usage: briefs/proof/5.4/prove.sh
# shellcheck disable=SC2016 # backticks in single quotes are Markdown code spans
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.4
work=$repo/web/app/dist/prove-5.4
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

# The screens' web unit and browser tests: the pictures below are only proof if they pass.
(cd web/app && "$repo/node_modules/.bin/vitest" run src/decisions src/timeline src/summary >/dev/null) \
  || miss "the screens' web unit tests failed"
(cd web/app && "$repo/node_modules/.bin/playwright" test e2e/decisions.spec.ts e2e/timeline.spec.ts e2e/summary.spec.ts --reporter=line >/dev/null) \
  || miss "the screens' browser tests failed"

# The pictures and the video.
pictures=(1-decision-view 2-partner-decision-revised 3-timeline-final-anchor 4-timeline-shortfall-and-why
  5-timeline-without-a-final-milestone 6-summary 7-summary-printed 8-dark-theme 9-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/views.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi

# The planted bug, in place, restored on any exit: the end anchor is the latest pin.
model=web/app/src/timeline/model.ts
cp "$model" "$work/model.ts"
restore() { cp "$work/model.ts" "$repo/$model"; }
trap restore EXIT
sed -i 's|  const entry = timeline.entries.find((each) => each.node === timeline.end);|  const entry = timeline.entries.filter((each) => each.origin === "pin").at(-1);|' "$model"
cmp -s "$model" "$work/model.ts" && miss "the planted bug was not planted"
status=0
planted=$(cd web/app && "$repo/node_modules/.bin/vitest" run src/timeline 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$model" "$work/model.ts" || miss "model.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"
caught=$(grep -E '^ *FAIL ' <<<"$planted" | sed 's/^ *//' | sort -u | head -n 6)
[ -n "$caught" ] || miss "the planted bug's failures were not found"
status=0
(cd web/app && "$repo/node_modules/.bin/vitest" run src/timeline >/dev/null 2>&1) || status=$?
[ "$status" -eq 0 ] || miss "the timeline's tests fail once the plant is removed"

out=$readme.tmp
cat >"$out" <<'EOF'
# Proof for brief 5.4: Decision view, timeline, status summary

## What you can do now

Beside a journey's canvas and its acting screens, three read-mostly screens, each a tab of the
journey and each opening a node's detail beside it:

- **Decisions** (C12): the journey's decisions on a canvas with the edges where one gates
  another, and a table of each answer in effect with what it affects: the nodes whose
  relevance it decides (and their relevance now), the milestone it pins, the role it fills.
  Revise an answer from the detail panel and the affected nodes change at once.
- **Timeline** (C13): every in-scope milestone, pin, and due date on one time axis with today
  marked; actual, pinned, and derived dates drawn apart; overdue and shortfall marked; the end
  anchored on the journey's final milestone. Each row's "Why" shows the date's chain and what
  to edit to move it (F7).
- **Summary** (C18): a printable status page for observers: counts by state, what remains,
  what is overdue, short, or stale and why, upcoming milestones with their dates and owners,
  and open decisions with their owners. Printing leaves out navigation, controls, and the
  panel, in black on white whatever the screen's theme.

The pictures are on the in-browser host, seeded from the fixtures, read on 2026-10-06.

## Decisions

The vendor evaluation: answering "no" to whether a partner runs the testing leaves the
partner-led work not relevant; the meeting date pins the decision meeting; three answers fill
roles. No decision here gates another, so the canvas sets them in a grid.

![The decision view](1-decision-view.png)

The partner decision revised to "yes" from its detail: the partner-led work is relevant at
once, and the notice names the work that became stale.

![An answer revised](2-partner-decision-revised.png)

## Timeline

The vendor evaluation ends at the decision meeting, its final milestone, pinned on
2026-11-20. Kickoff is an actual date, the final report a pin, the rest derived.

![Anchored on the final milestone](3-timeline-final-anchor.png)

The product launch's code freeze was reached later than the chain from the launch pin
allows: it and the launch are 2 days short. Its "Why" shows the chain and the moves that
would resolve it. The retrospective has no date yet.

![A shortfall and why](4-timeline-shortfall-and-why.png)

The hiring loop has no final milestone. Once the offer letter is pinned it is on the
timeline, with no end anchor.

![No final milestone](5-timeline-without-a-final-milestone.png)

## Summary

The product launch, with its two shortfalls and three upcoming milestones.

![The summary](6-summary.png)

The bake-off's summary printed with a node's detail open.

![Printed](7-summary-printed.png)

## Dark theme and a narrow screen

![Dark theme](8-dark-theme.png)

On a phone-sized screen each screen fits its width; the timeline puts each label above its
track.

![Narrow screen](9-narrow-screen.png)

The main flow, from the canvas through the decisions (an answer revised), the timeline (a
row's chain), and the summary: [main-flow.webm](main-flow.webm).

## Known limits

- No fixture has a decision gated by another, so none of these pictures shows the decision
  canvas's layered layout with edges.
- Without a final milestone the timeline has no named end; it simply ends after its latest
  date (`decisions/2026-10-07-the-timelines-end-anchor-is-the-final-milestone-alone.md`).
- The timeline and the decision view are read-only: a date or an answer changes in the
  node's detail. A markdown export of the summary is Later in the PRD.

Generated by `briefs/proof/5.4/prove.sh`, which also runs the screens' tests.
EOF
mv "$out" "$readme"
if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcomes differ from those expected" >&2
  exit 1
fi
echo "prove.sh: wrote $readme"
