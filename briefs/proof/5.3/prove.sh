#!/usr/bin/env bash
# Generates briefs/proof/5.3/: the README, a screenshot of each acceptance state of the list,
# the next list, triage, and the decision walkthrough, a short video of the walkthrough, and
# the values they showed. Builds the module (mise run build:wasm) and the fixture server, runs
# the acting surfaces' web unit tests (Vitest) and browser tests (Playwright,
# e2e/acting.spec.ts in rung 6's web/app suite) keeping their counts, runs the proof's pictures
# (web/app/proof/acting.proof.ts), then the planted bug: a bulk action sent as one patch per
# node (in `runBulk`, the bulk bar's one way to send), caught by C9's one-patch unit test.
# The plant is made in place and always restored (checked byte for byte). Exits non-zero if any outcome differs from the one expected.
#
# usage: briefs/proof/5.3/prove.sh
# shellcheck disable=SC2016 # backticks in single quotes are Markdown code spans
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.3
work=$repo/web/app/dist/prove-5.3
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

# The acting surfaces' web unit tests: each file and its count.
unit=$work/vitest.json
(cd web/app && "$repo/node_modules/.bin/vitest" run src/acting --reporter=json --outputFile.json="$unit" >/dev/null) \
  || miss "the acting surfaces' web unit tests failed"
unit_rows=$(node -e '
  const report = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  for (const file of report.testResults) {
    const name = file.name.replace(/.*\/web\/app\//, "");
    const passed = file.assertionResults.filter((test) => test.status === "passed").length;
    console.log(`| \`${name}\` | ${passed} passed, ${file.assertionResults.length - passed} failed |`);
  }' "$unit")

# The acting surfaces' browser tests: each test and its outcome.
e2e=$work/playwright.json
(cd web/app && PLAYWRIGHT_JSON_OUTPUT_NAME=$e2e "$repo/node_modules/.bin/playwright" test e2e/acting.spec.ts --reporter=json >/dev/null) \
  || miss "the acting surfaces' browser tests failed"
e2e_rows=$(node -e '
  const report = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const walk = (suite) => [
    ...(suite.specs ?? []).map((spec) => `| ${spec.title} | ${spec.ok ? "passed" : "failed"} |`),
    ...(suite.suites ?? []).flatMap(walk),
  ];
  for (const suite of report.suites) for (const row of walk(suite)) console.log(row);
' "$e2e")

# The pictures, and the values they showed.
pictures=(1-walkthrough-at-the-start 2-the-answer-surfaces-partner-work 3-triage-every-kind 4-placeholder-card
  5-what-would-unblock-the-next-decisions 6-next-ranked-with-why 7-next-re-sorted-by-slack 8-stalled-with-unsnooze
  9-list-filtered-and-grouped 10-list-search-reads-notes 11-bulk-done-rejected-naming-the-node
  12-bulk-snoozed-in-one-patch 13-dark-theme 14-narrow-screen)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/acting.proof.ts --reporter=line) \
  || miss "the proof run failed"
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else miss "the picture $picture is missing"; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else miss "the main flow's video is missing"; fi
tables=$(node -e '
  const values = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const code = (key) => `\`${key}\``;
  const out = [];
  const expect = (ok, what) => { if (!ok) { console.error(what); process.exitCode = 1; } };
  const upFront = ["n_meeting_date", "n_partner_runs", "n_purpose", "n_who_informed", "n_who_owns"];
  expect(JSON.stringify([...values.walkthroughStart].sort()) === JSON.stringify(upFront), "the walkthrough did not open on the up-front decisions");
  expect(values.surfacedByPartner.includes("n_criteria"), "answering the partner decision surfaced no partner-led work");
  expect(values.bulkRejected.before === values.bulkRejected.after, "a rejected bulk patch moved the revision");
  expect(values.bulkSnooze.after === values.bulkSnooze.before + 1, "a bulk snooze took more than one patch");
  const slacks = values.nextBySlack.map((row) => row.slack).filter((slack) => slack !== "").map(Number);
  expect(slacks.every((slack, at) => at === 0 || slacks[at - 1] <= slack), "the slack sort is not ascending");
  out.push(`- The walkthrough at the start, in pass order: ${values.walkthroughStart.map(code).join(", ")}.`);
  out.push(`- Surfaced by answering the partner decision yes: ${values.surfacedByPartner.map(code).join(", ")}.`);
  out.push(`- Triage of every kind in the same pass: ${values.triageAfterPartner.map(code).join(", ")}.`);
  out.push(`- With the five decisions skipped, what the walkthrough says would unblock the next ones: ${values.waiting.map((each) => `${code(each.decision)} (${each.unblockers.map(code).join(", ")})`).join("; ")}.`);
  out.push(`- A bulk done over the final report and the decision meeting: rejected, revision ${values.bulkRejected.before} before and ${values.bulkRejected.after} after.`);
  out.push(`- A bulk snooze of two nodes: revision ${values.bulkSnooze.before} to ${values.bulkSnooze.after}, one patch.`);
  out.push("");
  out.push("The product launch'"'"'s next list, by rank and by slack:");
  out.push("");
  out.push("| By rank | Rank | Slack | By slack | Slack |");
  out.push("|---|---|---|---|---|");
  values.nextRanked.forEach((row, at) => {
    const other = values.nextBySlack[at] ?? { key: "", slack: "" };
    out.push(`| ${code(row.key)} | ${Number(row.rank).toFixed(4)} | ${row.slack || "none"} | ${code(other.key)} | ${other.slack || "none"} |`);
  });
  console.log(out.join("\n"));' "$work/acting.json") || miss "the proof's values are not the ones expected"

# The planted bug, in place, restored on any exit: a bulk action sent as one patch per node.
acts=web/app/src/acting/acts.ts
cp "$acts" "$work/acts.ts"
restore() { cp "$work/acts.ts" "$repo/$acts"; }
trap restore EXIT
sed -i 's|^  return { outcome: "sent", landed: await run(plan.mutations) };$|  return { outcome: "sent", landed: (await Promise.all(plan.mutations.map((mutation) => run([mutation])))).every(Boolean) };|' "$acts"
cmp -s "$acts" "$work/acts.ts" && miss "the planted bug was not planted"
status=0
planted=$(cd web/app && "$repo/node_modules/.bin/vitest" run src/acting/acts.test.ts 2>&1) || status=$?
restore
trap - EXIT
cmp -s "$acts" "$work/acts.ts" || miss "acts.ts was not restored"
[ "$status" -ne 0 ] || miss "the planted bug went unnoticed"
caught=$(grep -E '(FAIL|AssertionError)' <<<"$planted" | sed 's/^ *//' | head -n 2 | paste -sd ' ' -)
[ -n "$caught" ] || miss "the planted bug's failure was not found"

out=$readme.tmp
{
  cat <<'EOF'
# Proof for brief 5.3: List, next, triage, decision walkthrough

Generated by `briefs/proof/5.3/prove.sh`; do not edit by hand. The script fails if any
outcome below differs from the one expected.

A journey now has three acting screens beside its canvas, linked from a row of journey
screens. The next list (C10) is the acting frontier in rank order, each item with its
breadcrumb, why it ranks where it does (the rank blend's parts), and its kind's actions inline;
it re-sorts by any single signal, ranks for the viewer ("prioritize for me") without touching
the shared rank, filters to only mine and by kind, offers "assign owner" on unassigned items
(D2), and, when nothing can be acted on, shows what the journey waits on with unsnooze (D5).
The list (C9) is the journey as a table with every filter, grouping by container, a sort by
one signal, and text search over titles, descriptions, notes, and resources; its selection's
bulk start, done, skip, assign, snooze, and unsnooze each go as one patch, one event per node,
and a guard failure on any node rejects the whole patch and names it. Triage (C11) is one card
at a time in rank order with each kind's actions and pass, which only reorders this pass and
writes nothing; a placeholder offers mark atomic and snooze and no done (B10). Its
decisions-only mode is the decision walkthrough, where a new journey lands: it names what an
answer surfaced in the same pass, and with no decision to make, what would unblock the next
ones.

The walkthrough's pictures are on the server host, over a journey started from version 1 of
the vendor evaluation's route; the rest are on the in-browser host, seeded from the fixtures
on each load.

## The decision walkthrough (C11)

A fresh evaluation opens on every decision actionable at its start: the four up-front
decisions and the partner decision, which leads on gravity (the partner-led work rides on it):

![The walkthrough at the start](1-walkthrough-at-the-start.png)

Answering that a partner runs the testing makes the partner-led work relevant and actionable,
and the walkthrough names it in the same pass:

![The answer surfaces partner work](2-the-answer-surfaces-partner-work.png)

Triage of every kind, in the same pass, holds it among the cards:

![Triage of every kind](3-triage-every-kind.png)

Once kickoff is reached, the test workload placeholder's card offers mark atomic and snooze,
and no done:

![A placeholder's card](4-placeholder-card.png)

With every decision skipped in one bulk patch, nothing can be decided, and the walkthrough
shows what would unblock the next decisions:

![What would unblock the next decisions](5-what-would-unblock-the-next-decisions.png)

## The next list (C10, D5)

The product launch's next list, each item with its breadcrumb and why it ranks there:

![Ranked](6-next-ranked-with-why.png)

Re-sorted by slack:

![By slack](7-next-re-sorted-by-slack.png)

The hiring loop's only item snoozed: the journey is stalled, and the panel offers unsnooze:

![Stalled](8-stalled-with-unsnooze.png)

## The list (C9)

Deliverables and actions, grouped by container:

![Filtered and grouped](9-list-filtered-and-grouped.png)

Search reads notes: "environment team" finds the node whose note says it:

![Search](10-list-search-reads-notes.png)

A bulk done over the final report and the decision meeting: the final report fails its
`deps_done` guard (its stage has not opened), so the whole patch is rejected and the rejection
names it, with its bypass:

![Rejected](11-bulk-done-rejected-naming-the-node.png)

Two nodes snoozed in one patch until the beta ends:

![Snoozed](12-bulk-snoozed-in-one-patch.png)

## A dark theme, a narrow screen

![Dark theme](13-dark-theme.png)

![Narrow screen](14-narrow-screen.png)

The main flow: a fresh journey's walkthrough, the partner decision answered and its work
surfacing, a pass, triage of every kind, and the next list: [main-flow.webm](main-flow.webm).

## The values shown

EOF
  printf '%s\n' "$tables"
  cat <<'EOF'

## The tests

The acting surfaces' web unit tests (Vitest, rung 6):

| File | Result |
|---|---|
EOF
  printf '%s\n' "$unit_rows"
  cat <<'EOF'

The acting surfaces' browser tests (Playwright in Chromium, rung 6, `web/app/e2e/acting.spec.ts`):

| Test | Result |
|---|---|
EOF
  printf '%s\n' "$e2e_rows"
  cat <<'EOF'

## The planted bug

A bulk action sent as one patch per node (`acts.ts`'s `runBulk`, through which the list's bulk
bar sends every bulk action, sends each mutation alone): C9's one-patch unit test fails:

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
