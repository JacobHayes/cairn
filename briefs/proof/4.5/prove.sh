#!/usr/bin/env bash
# Prints the derive benchmark table of briefs/proof/4.5/README.md: the document at the limits
# read and derived natively (the cairn-wasm-cases binary, optimized) and in Chromium's derive
# worker (the browser benchmark test), as a Markdown table on stdout for the README.
#
# usage: briefs/proof/4.5/prove.sh WORK_DIR
#
# WORK_DIR holds what the browser benchmark found; the repository is never modified, apart
# from the build outputs under web/wasm/dist/.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
work=${1:?usage: prove.sh WORK_DIR}
mkdir -p "$work"
work=$(cd "$work" && pwd)
export CARGO_TERM_COLOR=never

cd "$repo"
found=$work/found
cases=$repo/web/wasm/dist/cases
rm -rf "$found"
mise run build:wasm >&2
mise exec -- cargo run --quiet --locked --release -p cairn-wasm --features server \
  --bin cairn-wasm-cases -- "$cases" --bench >&2
(cd web/wasm && CAIRN_PROOF_OUT=$found "$repo/node_modules/.bin/playwright" test \
  -g "the derive benchmark at the limits" --reporter=line >&2)
node web/wasm/e2e/proof.ts "$found" "$cases"
