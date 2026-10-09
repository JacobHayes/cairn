#!/usr/bin/env bash
# Regenerates the media of briefs/proof/7.4/: a screenshot of each state of snoozing a
# container from its detail (web/app/proof/snooze.proof.ts, which asserts what each shows),
# and prints the next list before, during and after as a Markdown table. Builds the module
# (mise run build:wasm) and the fixture server first. The README is written by hand.
#
# usage: briefs/proof/7.4/prove.sh
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
proof=$repo/briefs/proof/7.4
work=$repo/web/app/dist/prove-7.4
cd "$repo"
rm -rf "$work"
mkdir -p "$work"

mise run build:wasm
mise exec -- cargo build --quiet --locked -p cairn-wasm --example fixture_server

pictures=(1-snoozing-a-group-from-its-detail 2-the-next-list-without-the-branch 3-a-descendant-names-its-container)
rm -f "$proof"/*.png
(cd web/app && CAIRN_PROOF_OUT=$work "$repo/node_modules/.bin/playwright" test -c playwright.proof.config.ts proof/snooze.proof.ts --reporter=line)
missing=0
for picture in "${pictures[@]}"; do
  if [ -s "$work/$picture.png" ]; then cp "$work/$picture.png" "$proof/"; else echo "prove.sh: the picture $picture is missing" >&2; missing=1; fi
done
[ "$missing" -eq 0 ]

# shellcheck disable=SC2016 # the backticks are a JavaScript template literal
node -e '
  const { before, during, after } = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const code = (keys) => keys.map((key) => "`" + key + "`").join(", ");
  console.log("| Next list | Items |");
  console.log("|---|---|");
  console.log(`| Before | ${code(before)} |`);
  console.log(`| Launch materials snoozed | ${code(during)} |`);
  console.log(`| Unsnoozed from a descendant | ${code(after)} |`);' "$work/snooze.json"
echo "prove.sh: wrote the media of $proof" >&2
