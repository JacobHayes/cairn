#!/usr/bin/env bash
# Regenerates the media of briefs/proof/5.3/: a screenshot of each acceptance state of the
# list, the next list, triage, and the decision walkthrough, and a short video of the
# walkthrough. Builds the module (mise run build:wasm) and the fixture server, runs the proof's
# pictures (web/app/proof/acting.proof.ts), which assert the state each picture shows, and
# prints the product launch's next list by rank and by slack as a Markdown table. Exits
# non-zero if the run fails or a picture is missing. The README is written by hand.
#
# usage: briefs/proof/5.3/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/5.3
work=$repo/web/app/dist/prove-5.3
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-walkthrough-at-the-start 2-the-answer-surfaces-partner-work 3-triage-every-kind 4-placeholder-card
  5-what-would-unblock-the-next-decisions 6-next-ranked-with-why 7-next-re-sorted-by-slack 8-stalled-with-unsnooze
  9-list-filtered-and-grouped 10-list-search-reads-notes 11-bulk-done-rejected-naming-the-node
  12-bulk-snoozed-in-one-patch)
rm -f "$proof"/*.png "$proof"/*.webm
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/acting.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
if [ -s "$work/main-flow.webm" ]; then cp "$work/main-flow.webm" "$proof/"; else echo "prove.sh: the main flow's video is missing" >&2; missing=1; fi
[ "$missing" -eq 0 ]

# shellcheck disable=SC2016 # the backticks are a JavaScript template literal
node -e '
  const values = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const code = (key) => "`" + key + "`";
  console.log("| By rank | Rank | Slack | By slack | Slack |");
  console.log("|---|---|---|---|---|");
  values.nextRanked.forEach((row, at) => {
    const other = values.nextBySlack[at] ?? { key: "", slack: "" };
    console.log(`| ${code(row.key)} | ${Number(row.rank).toFixed(4)} | ${row.slack || "none"} | ${code(other.key)} | ${other.slack || "none"} |`);
  });' "$work/acting.json"
echo "prove.sh: wrote the media of $proof" >&2
