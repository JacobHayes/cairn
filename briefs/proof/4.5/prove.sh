#!/usr/bin/env bash
# Prints the derive benchmark table of briefs/proof/4.5/README.md: the document at the limits
# read and derived natively (the cairn-wasm-cases binary, optimized) and by the derive worker's
# own script over the wasm32 module in Node (web/wasm/proof/bench.ts), as a Markdown table on
# stdout for the README, with the graph's size figures over a generated 2,000-node journey.
#
# usage: briefs/proof/4.5/prove.sh
#
# The repository is never modified, apart from the build outputs under web/wasm/dist/.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
export CARGO_TERM_COLOR=never

cd "$repo"
cases=$repo/web/wasm/dist/cases
mise run build:wasm >&2
mise exec -- cargo run --quiet --locked --release -p cairn-wasm --features server \
  --bin cairn-wasm-cases -- "$cases" --bench >&2
# The release run replaced the cases the tests read with its own (the same answers).
node web/wasm/proof/bench.ts "$cases"
