#!/usr/bin/env bash
# Generates briefs/proof/1.3/README.md: the simulation spike (testbeds/spike) built
# shim-linked and run under patina at one seed, fault-free and then with one injected fault
# at a time, with and without the planted server bug. Exits non-zero if any run's outcome
# differs from the one it expects.
#
# usage: briefs/proof/1.3/prove.sh
#
# Needs cargo-patina (mise install). Writes only the README and the spike's target directory.
# shellcheck disable=SC2016 # backticks in single quotes are Markdown code spans
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
spike_dir=$repo/testbeds/spike
readme=$repo/briefs/proof/1.3/README.md
export CARGO_TERM_COLOR=never
mismatches=0

cd "$spike_dir"
cargo build --locked --quiet # creates the managed target directory
mkdir -p target/patina
spike=target/patina/cairn-spike
cargo patina build . --output "$spike" >target/patina/proof-build.log 2>&1 || {
  cat target/patina/proof-build.log >&2
  exit 1
}

# field NAME LINE: the value of NAME=value in LINE.
field() { grep -o "$1=[0-9a-z]*" <<<"$2" | head -1 | cut -d= -f2; }

# scenario TITLE EXPECTED_EXIT CHECK DESCRIPTION RUN_ARGS... [-- SPIKE_ARGS...]
# CHECK is a shell condition over $retries, $deduplicated, $elapsed, and $log.
rows=()
sections=()
scenario() {
  local title=$1 expected=$2 check=$3 description=$4
  shift 4
  local log status=0 result retries deduplicated elapsed verdicts
  log=$(cargo patina run "$spike" --seed 1 "$@" 2>&1 >/dev/null) || status=$?
  result=$(grep '^SPIKE_RESULT ' <<<"$log" || true)
  retries=$(field retries "$result")
  deduplicated=$(field deduplicated "$result")
  elapsed=$(field elapsed_us "$result")
  verdicts=$(grep -o '^PATINA_VERDICT .* kind=[a-z_]* label=[a-z-]*' <<<"$log" |
    sed 's/.*kind=\([a-z_]*\) label=\(.*\)/\1 \2/' | sort | uniq -c |
    awk '{ printf "%s%s %s x%s", sep, $2, $3, $1; sep = ", " }')
  if [ "$status" -ne "$expected" ] || ! eval "$check"; then
    mismatches=$((mismatches + 1))
    echo "prove.sh: MISMATCH in '$title': exit $status (expected $expected), check: $check" >&2
    printf '%s\n' "$log" >&2
  fi
  rows+=("| $title | $status | $retries | $deduplicated | $elapsed | $verdicts |")
  sections+=("$(
    printf '## %s\n\n%s\n\n```\n$ cargo patina run target/patina/cairn-spike --seed 1 %s\nexit status: %s (expected %s)\n' \
      "$title" "$description" "$*" "$status" "$expected"
    # The first few retries, then every verdict and the result, then the fault and fault-site
    # reports' counters (the reports are long; these are the fields that show what fired).
    grep '^SPIKE_RETRY ' <<<"$log" | head -3 || true
    retried=$(grep -c '^SPIKE_RETRY ' <<<"$log" || true)
    [ "$retried" -le 3 ] || printf '... %s SPIKE_RETRY lines in all\n' "$retried"
    grep -E '^(PATINA_VERDICT|SPIKE_VIOLATION|SPIKE_RESULT|SPIKE_FAILURE)' <<<"$log" || true
    if grep -q '^PATINA_NET_FAULT_REPORT ' <<<"$log"; then
      grep '^PATINA_NET_FAULT_REPORT ' <<<"$log" |
        grep -oE '^PATINA_NET_FAULT_REPORT|(send_ops|drops_applied|jitter_applied|latency_applied|connects_refused|resets_injected|vacuous)=[0-9]+' |
        paste -sd' '
    fi
    grep '^PATINA_SDK_REPORT ' <<<"$log" |
      grep -oE '^PATINA_SDK_REPORT|(enabled|sites_activated|total_firings)=[0-9]+' | paste -sd' ' || true
    printf '```\n'
  )")
}

scenario "Fault-free" 0 '[ "$retries" = 0 ] && [ "$deduplicated" = 0 ]' \
  "No fault: every request succeeds on its first attempt." \
  -- --tick-ms 1
scenario "Connections reset" 0 '[ "$retries" -gt 0 ] && [ "$deduplicated" -gt 0 ]' \
  "Each established stream operation is reset with probability 10%. Clients retry; requests whose answer was lost after the server applied them come back as retries the server deduplicates." \
  --net-reset-permille 100 -- --tick-ms 1
scenario "Connections refused" 0 '[ "$retries" -gt 0 ] && [ "$deduplicated" = 0 ]' \
  "Each connect is refused with probability 20%. Clients retry; a refused request never reached the server, so nothing is deduplicated." \
  --net-connect-refuse-permille 200 -- --tick-ms 1
scenario "Connections delayed" 0 '[ "$elapsed" -gt 0 ]' \
  "Every segment is delayed by 2 ms plus up to 2 ms of jitter, and 10% are retransmitted after a backoff. The same requests succeed, later in virtual time." \
  --net-latency-nanos 2000000 --net-jitter-nanos 0..2000000 --net-drop-permille 100 -- --tick-ms 1
scenario "Responses lost (fault site)" 0 '[ "$retries" -gt 0 ] && [ "$deduplicated" -gt 0 ]' \
  "The client-side \`buggify!\` site \`client-loses-response\`, armed on every first attempt: the client drops the server's answer and retries." \
  --buggify=1000 --buggify-activation-permille 1000 -- --tick-ms 1
scenario "Planted bug, fault-free" 0 '[ "$retries" = 0 ]' \
  "\`--bug no-dedupe\`: the server appends a retried request again. Without a fault nothing is retried, so the bug is invisible." \
  -- --tick-ms 1 --bug no-dedupe
scenario "Planted bug, connections reset" 1 'grep -q "label=applied-once" <<<"$log"' \
  "The same bug with connection resets: the retries the faults force are appended again, and the \`applied-once\` invariant reports a violation for each request applied more than once." \
  --net-reset-permille 100 -- --tick-ms 1 --bug no-dedupe

{
  cat <<'EOF'
# Proof for brief 1.3: the simulation spike

Generated by `briefs/proof/1.3/prove.sh`; do not edit by hand. Each row is one run of the
spike (`testbeds/spike`: an axum server and three hyper clients on one current-thread tokio
runtime) under the patina native shim, at seed 1, through `cargo patina run`. The spike
reports its outcome through patina's verdict channel; `retries`, `deduplicated`, and
`elapsed_us` (virtual microseconds) come from its result line. The script fails if any run's
outcome differs from the one expected.

What this shows: tokio, axum, and an HTTP client run under the shim; injected faults reach
the program and visibly change a run's outcome (retries and deduplicated requests appear,
virtual time stretches); and a server bug that a fault-free run cannot see is caught by an
invariant once a fault forces a retry. What does not work under the shim is in
`testbeds/spike/README.md`, Patina gap report.

| Run | Exit | Retries | Deduplicated | Elapsed (us) | Verdicts |
|---|---|---|---|---|---|
EOF
  printf '%s\n' "${rows[@]}"
  for section in "${sections[@]}"; do printf '\n%s\n' "$section"; done
} >"$readme"

if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches scenario(s) did not match" >&2
  exit 1
fi
echo "prove.sh: wrote $readme"
