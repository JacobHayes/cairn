#!/usr/bin/env bash
# Generates briefs/proof/6.1/README.md: the multiplayer testbed (testbeds/multiplayer) built
# shim-linked and run under patina: one seed fault-free and under each injected fault, the
# smoke campaign's generations and oracle coverage, and the product finding it makes,
# minimized to the knob that matters and rewritten as a deterministic service test. Exits
# non-zero if any outcome differs from the one it expects.
#
# usage: briefs/proof/6.1/prove.sh
#
# Needs cargo-patina (mise install). Writes only the README and the testbed's target
# directory.
# shellcheck disable=SC2016 # backticks in single quotes are Markdown code spans
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
testbed_dir=$repo/testbeds/multiplayer
readme=$repo/briefs/proof/6.1/README.md
export CARGO_TERM_COLOR=never
mismatches=0
mismatch() {
  mismatches=$((mismatches + 1))
  echo "prove.sh: MISMATCH: $*" >&2
}

cd "$testbed_dir"
cargo build --locked --quiet # creates the managed target directory
out=target/patina/proof
mkdir -p "$out"
testbed=target/patina/cairn-multiplayer
cargo patina build . --output "$testbed" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  exit 1
}
allow=$(paste -sd, unsupported-symbols.txt)
shown_allow='--allow-unsupported-symbols "$(paste -sd, unsupported-symbols.txt)"'

# field NAME LINE: the value of NAME=value in LINE.
field() { grep -oE "(^| )$1=[0-9A-Za-z_]*" <<<"$2" | head -1 | cut -d= -f2; }
# verdicts LOG: each verdict kind and label, counted.
verdicts() {
  grep -oE '^PATINA_VERDICT .* kind=[a-z_]+ label=[a-z-]+' <<<"$1" |
    sed -E 's/.*kind=([a-z_]+) label=(.*)/\1 \2/' | sort | uniq -c |
    awk '{ printf "%s%s %s", sep, $2, $3; sep = ", " }'
}

# One seed under each fault: TITLE EXPECTED_EXIT CHECK DESCRIPTION RUN_ARGS...; CHECK is a
# shell condition over the result's fields as shell variables.
rows=()
sections=()
scenario() {
  local title=$1 expected=$2 check=$3 description=$4
  shift 4
  local log status=0 result
  log=$(cargo patina run "$testbed" --seed 1 --allow-unsupported-symbols "$allow" "$@" 2>&1 >/dev/null) || status=$?
  result=$(grep '^MULTIPLAYER_RESULT ' <<<"$log" || true)
  local acknowledged surfaced exhausted resubmitted receipts resent writes
  acknowledged=$(field acknowledged "$result")
  surfaced=$(field surfaced "$result")
  exhausted=$(field surfaced_exhausted "$result")
  resubmitted=$(field resubmitted "$result")
  receipts=$(field receipts "$result")
  resent=$(field transport_retries "$result")
  writes=$(field writes_us "$result")
  if [ "$status" -ne "$expected" ] || ! eval "$check"; then
    mismatch "'$title': exit $status (expected $expected), check: $check"
    printf '%s\n' "$log" | grep -E '^(MULTIPLAYER|PATINA_VERDICT)' >&2 || true
  fi
  rows+=("| $title | $status | $acknowledged | $surfaced ($exhausted) | $resubmitted | $receipts | $resent | $writes | $(verdicts "$log") |")
  sections+=("$(
    printf '### %s\n\n%s\n\n```\n$ cargo patina run target/patina/cairn-multiplayer --seed 1 %s %s\nexit status: %s (expected %s)\n' \
      "$title" "$description" "$shown_allow" "$*" "$status" "$expected"
    grep -E '^(PATINA_VERDICT|MULTIPLAYER_)' <<<"$log" | cut -c1-240 || true
    if grep -q '^PATINA_NET_FAULT_REPORT ' <<<"$log"; then
      grep '^PATINA_NET_FAULT_REPORT ' <<<"$log" |
        grep -oE '^PATINA_NET_FAULT_REPORT|(send_ops|drops_applied|latency_applied|connects_refused|resets_injected|vacuous)=[0-9]+' |
        paste -sd' '
    fi
    grep '^PATINA_SDK_REPORT ' <<<"$log" | grep -oE 'site=[a-z-]+\|fault\|a1\|e[0-9]+\|f[1-9][0-9]*' || true
    printf '```\n'
  )")
}

scenario "Fault-free" 0 '[ "$resent" = 0 ] && [ "$receipts" = 0 ]' \
  "No fault: every request is answered on its first attempt. Clients still conflict with each other, so some stale patches are resubmitted on their own (H5) and some overlapping renames are surfaced." \
  --
scenario "Connections reset" 0 '[ "$resent" -gt 0 ] && [ "$receipts" -gt 0 ]' \
  "Each established stream operation is reset with probability 10%. Clients resend; a patch whose answer was lost after it landed is answered from its receipt." \
  --net-reset-permille 100 --
scenario "Connections refused" 0 '[ "$resent" -gt 0 ] && [ "$receipts" = 0 ]' \
  "Each connect is refused with probability 20%. Clients resend; a refused request never reached the server, so nothing is answered from a receipt." \
  --net-connect-refuse-permille 200 --
scenario "Delayed and dropped segments" 0 '[ "$writes" -gt 100000 ]' \
  "2 ms latency, up to 2 ms jitter, and 10% of segments dropped: every exchange takes longer, so the writes take longer to finish." \
  --net-latency-nanos 2000000 --net-jitter-nanos 0..2000000 --net-drop-permille 100 --
scenario "Answers lost and commits held (buggify)" 0 '[ "$receipts" -gt 0 ] && [ "$resubmitted" -gt 0 ]' \
  "Every fault site active: clients drop first answers (\`client-loses-response\`), and commits wait before they begin and stall before they commit (\`store-commit-*\`, testbeds/multiplayer/src/faults.rs). Dropped answers come back from receipts; held commits make more patches stale." \
  --buggify=300 --buggify-activation-permille 1000 --

# The smoke campaign, as `mise run sim` runs it, generation by generation.
rm -rf "$out/campaign"
campaign_log=$(cargo patina campaign "$testbed" --gens 16 --buggify --sched-pct --out-dir "$out/campaign" \
  --progress-every 1 --allow-unsupported-symbols "$allow" 2>&1) || true
# The first violation each failing generation reported, by generation.
violated=$(node -e '
  for (const run of require(process.argv[1]).notable_runs) {
    const label = (run.signature.match(/label=([a-z-]+)/) || [])[1];
    if (label) console.log(`${run.generation} ${label}`);
  }' "$testbed_dir/$out/campaign/campaign-state.json")
generation_rows=$(grep '^PATINA_CAMPAIGN_GEN ' <<<"$campaign_log" | while read -r line; do
  generation=$(field generation "$line")
  label=$(awk -v generation="$generation" '$1 == generation { print $2 }' <<<"$violated")
  printf '| %s | %s | %s | %s |\n' "$generation" "$(field seed "$line")" "$(field class "$line")" "$label"
done)
coverage=$(grep -E '^PATINA_CAMPAIGN_COVERAGE ' <<<"$campaign_log" || true)
grep -q 'gate=pass' <<<"$coverage" || mismatch "campaign coverage gate: $coverage"
site_rows=$(node -e '
  const sites = require(process.argv[1]).sites;
  for (const site of sites.sort((a, b) => a.kind.localeCompare(b.kind) || a.label.localeCompare(b.label))) {
    const fired = site.kind === "fault" ? `fired in ${site.runs_fired}` : `satisfied in ${site.satisfied_gens}`;
    console.log(`| \`${site.label}\` | ${site.kind} | ${fired} of ${site.registered_gens} reached |`);
  }' "$testbed_dir/$out/campaign/sites.json")
failing=$(grep -oE 'first_gen=[0-9]+' <<<"$campaign_log" | head -1 | cut -d= -f2 || true)
[ -n "$failing" ] || mismatch "the campaign found no failing generation to minimize"
violation_classes=$(grep -oE 'label=[a-z-]+' <<<"$(grep 'signature: VIOLATION' <<<"$campaign_log")" | sort -u | cut -d= -f2 | paste -sd' ')
[ "$violation_classes" = "nothing-unacknowledged-applied" ] ||
  mismatch "campaign violations other than the known finding: $violation_classes"

# The finding, minimized: the knobs first, then the repro run.
minimized=$(cargo patina minimize --generation "$failing" --out-dir "$out/campaign" --no-trace-phase 2>&1) ||
  mismatch "minimize --generation $failing failed"
repro=$(cat "$out/campaign/minimized/generation-$failing.repro")
needs=$(grep -oE 'the failure needs only: .*' <<<"$minimized" || true)
read -r -a repro_args <<<"${repro#cargo patina run target/patina/cairn-multiplayer }"
repro_status=0
repro_log=$(cargo patina run "$testbed" "${repro_args[@]}" 2>&1 >/dev/null) || repro_status=$?
[ "$repro_status" -eq 1 ] || mismatch "the minimized repro exited $repro_status"
shown_repro=$(sed -E "s/--allow-unsupported-symbols [^ ]+/$shown_allow/" <<<"$repro")
test_status=0
test_log=$(cd "$repo" && cargo test --locked -q -p cairn-service --test in_flight -- --ignored \
  a_resubmission_while_its_original_commits 2>&1) || test_status=$?
[ "$test_status" -ne 0 ] || mismatch "the ignored regression test passed: the finding is fixed, update DECISIONS.md"

{
  cat <<'EOF'
# Proof: brief 6.1, the multiplayer testbed

Generated by `briefs/proof/6.1/prove.sh`; every run below is real output from patina at the
pinned revision. Before this brief nothing drove Cairn's HTTP server with more than one client
at a time; now `mise run sim` runs four HTTP clients and one in-process agent patching one
journey through Cairn's real server, store (Turso), and Rust client (H5's safe retry, H6's
subscription tracking), under seeded network faults and fault sites, checks nine invariants
at the end of every run, and found a product bug (DECISIONS.md, brief 6.1).

The `--allow-unsupported-symbols` list names exactly the symbols Turso links that the shim
refuses (testbeds/multiplayer/README.md, gap 4).

## Injected faults change the outcome

Seed 1, one fault at a time. *Surfaced* is the patches surfaced as conflicts, with how many of
those only because the resubmission bound ran out (nothing overlapping intervened);
*resubmitted* counts automatic resubmissions (H5); *receipts* the answers from a receipt;
*resent* the requests sent again after a lost answer; *writes* the virtual time until the last
client's last answer.

| Run | Exit | Acknowledged | Surfaced (bound) | Resubmitted | Receipts | Resent | Writes (us) | Verdicts |
|---|---|---|---|---|---|---|---|---|
EOF
  printf '%s\n' "${rows[@]}"
  printf '\n'
  printf '%s\n\n' "${sections[@]}"
  cat <<EOF
## The smoke campaign

\`cargo patina campaign target/patina/cairn-multiplayer --gens 16 --buggify --sched-pct\`, as
\`mise run sim\` runs it: each generation draws its seed, buggify rates, and PCT depth from its
number. Every failing generation breaks one invariant, the known product finding below; sim.sh
fails on any other.

| Generation | Seed | Class | Violation |
|---|---|---|---|
$generation_rows

\`$coverage\`

Every coverage oracle declared in the binary fired, the service's and client's own
\`reachable!\` sites included; the fault sites are listed by the runs they fired in:

| Site | Kind | Generations |
|---|---|---|
$site_rows

## The product finding, minimized

Generation $failing reduced to the knobs it needs (\`cargo patina minimize --generation $failing
--no-trace-phase\`): $needs

\`\`\`
\$ $shown_repro
exit status: $repro_status
$(grep -E '^(PATINA_VERDICT|MULTIPLAYER_)' <<<"$repro_log" | cut -c1-240)
\`\`\`

The agent's commit stalled past its 200 ms attempt timeout; the agent resubmitted the same
patch while the original was still committing; the store answered the resubmission stale,
naming the revision the original was producing, with nothing intervening; the client rebased
and resubmitted until its bound ran out and surfaced the conflict; then the original landed.
The caller was told a patch conflicted that is in the journey. Rewritten as a deterministic
service test with the commit held open at a gate (\`crates/service/tests/in_flight.rs\`, ignored
until fixed):

\`\`\`
\$ cargo test -p cairn-service --test in_flight -- --ignored a_resubmission_while_its_original_commits
$(grep -E '^(test result| +(left|right):)' <<<"$test_log" | cut -c1-240)
\`\`\`
EOF
} >"$readme"

if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcome(s) differed from what was expected" >&2
  exit 1
fi
echo "prove.sh: wrote $readme"
