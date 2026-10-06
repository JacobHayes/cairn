#!/usr/bin/env bash
# Generates briefs/proof/6.1/README.md: the multiplayer testbed (testbeds/multiplayer) built
# shim-linked and run under patina: one seed fault-free and under each injected fault, the
# smoke campaign's generations and oracle coverage, and the planted bug (Turso's revision
# check skipping the target domain) found by the campaign, minimized to the knobs that
# matter, and shown by its deterministic service test. The product finding 6.1 made is
# fixed (DECISIONS.md); its service tests are shown passing. Exits non-zero if any outcome
# differs from the one it expects.
#
# usage: briefs/proof/6.1/prove.sh
#
# Needs cargo-patina (mise install). Writes the README and the testbed's target directory.
# For the planted build it plants the bug in crates/store-turso/src/commit.rs and restores
# the file however the script ends.
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

# The oracles sim.sh lists as out of this testbed's reach: the only ones allowed unmet.
out_of_reach=$(sed -n '/^out_of_reach=(/,/^)/p' sim.sh | grep -E '^  [a-z-]+$' | tr -d ' ' | sort)

# campaign BINARY DIR: the smoke campaign as `mise run sim` runs it, generation by generation.
campaign() {
  rm -rf "$2"
  cargo patina campaign "$1" --gens 16 --buggify --sched-pct --out-dir "$2" \
    --progress-every 1 --allow-unmet-sometimes --allow-unsupported-symbols "$allow" 2>&1 || true
}
# generation_rows LOG DIR: a table row per generation, with the first violation it reported.
generation_rows() {
  local violated
  violated=$(node -e '
    for (const run of require(process.argv[1]).notable_runs) {
      const label = (run.signature.match(/label=([a-z-]+)/) || [])[1];
      if (label) console.log(`${run.generation} ${label}`);
    }' "$testbed_dir/$2/campaign-state.json")
  grep '^PATINA_CAMPAIGN_GEN ' <<<"$1" | while read -r line; do
    generation=$(field generation "$line")
    label=$(awk -v generation="$generation" '$1 == generation { print $2 }' <<<"$violated")
    printf '| %s | %s | %s | %s |\n' "$generation" "$(field seed "$line")" "$(field class "$line")" "$label"
  done
}

# At head: every generation passes, and every oracle in reach fires.
campaign_log=$(campaign "$testbed" "$out/campaign")
head_rows=$(generation_rows "$campaign_log" "$out/campaign")
coverage=$(grep -E '^PATINA_CAMPAIGN_COVERAGE ' <<<"$campaign_log" || true)
unmet=$(grep -oE "^  UNMET [a-z]+ '[^']+'" <<<"$campaign_log" | sed -E "s/.*'(.*)'/\1/" | sort -u || true)
[ "$unmet" = "$out_of_reach" ] || mismatch "unmet oracles '$unmet', expected those out of reach '$out_of_reach'"
if grep -q 'signature: VIOLATION' <<<"$campaign_log"; then
  mismatch "the campaign at head found a violation"
fi
site_rows=$(node -e '
  const sites = require(process.argv[1]).sites;
  for (const site of sites.sort((a, b) => a.kind.localeCompare(b.kind) || a.label.localeCompare(b.label))) {
    const fired = site.kind === "fault" ? `fired in ${site.runs_fired}` : `satisfied in ${site.satisfied_gens}`;
    console.log(`| \`${site.label}\` | ${site.kind} | ${fired} of ${site.registered_gens} reached |`);
  }' "$testbed_dir/$out/campaign/sites.json")
fixed_log=$(cd "$repo" && cargo test --locked -q -p cairn-service --test in_flight 2>&1) ||
  mismatch "the in-flight service tests fail at head"

# The planted bug: Turso's commit-time revision check skips the target domain.
planted_file=$repo/crates/store-turso/src/commit.rs
cp "$planted_file" "$out/commit.rs.unplanted"
restore() { cp "$out/commit.rs.unplanted" "$planted_file"; }
trap restore EXIT
sed -i '0,/        if now != expected {/s//        if index > 0 \&\& now != expected {/' "$planted_file"
planted_diff=$(diff "$out/commit.rs.unplanted" "$planted_file" || true)
[ -n "$planted_diff" ] || mismatch "the plant did not apply"
planted=target/patina/cairn-multiplayer-planted
cargo patina build . --output "$planted" >"$out/build-planted.log" 2>&1 || {
  cat "$out/build-planted.log" >&2
  exit 1
}
planted_log=$(campaign "$planted" "$out/planted")
planted_rows=$(generation_rows "$planted_log" "$out/planted")
failing=$(grep -oE 'first_gen=[0-9]+' <<<"$planted_log" | head -1 | cut -d= -f2 || true)
[ -n "$failing" ] || mismatch "the planted campaign found no failing generation to minimize"
minimized=$(cargo patina minimize --generation "$failing" --out-dir "$out/planted" --no-trace-phase 2>&1) ||
  mismatch "minimize --generation $failing failed"
repro=$(cat "$out/planted/minimized/generation-$failing.repro")
needs=$(grep -oE 'the failure needs only: .*' <<<"$minimized" || true)
read -r -a repro_args <<<"${repro#cargo patina run target/patina/cairn-multiplayer-planted }"
repro_status=0
repro_log=$(cargo patina run "$planted" "${repro_args[@]}" 2>&1 >/dev/null) || repro_status=$?
[ "$repro_status" -eq 1 ] || mismatch "the minimized repro exited $repro_status"
shown_repro=$(sed -E "s/--allow-unsupported-symbols [^ ]+/$shown_allow/" <<<"$repro")
race_test=a_patch_that_loses_the_race_to_commit_is_rejected_stale_on_turso
test_status=0
test_log=$(cd "$repo" && cargo test --locked -q -p cairn-service --test in_flight -- "$race_test" 2>&1) || test_status=$?
[ "$test_status" -ne 0 ] || mismatch "the race test passed with the bug planted"
restore
trap - EXIT
unplanted_log=$(cd "$repo" && cargo test --locked -q -p cairn-service --test in_flight -- "$race_test" 2>&1) ||
  mismatch "the race test fails with the plant removed"

{
  cat <<'EOF'
# Proof: brief 6.1, the multiplayer testbed

Generated by `briefs/proof/6.1/prove.sh`; every run below is real output from patina at the
pinned revision. Before this brief nothing drove Cairn's HTTP server with more than one client
at a time; now `mise run sim` runs four HTTP clients and one in-process agent patching one
journey through Cairn's real server, store (Turso), and Rust client (H5's safe retry, H6's
subscription tracking), under seeded network faults and fault sites, checks nine invariants
at the end of every run, and found a product bug, since fixed (DECISIONS.md, brief 6.1).

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
number. Every generation passes.

| Generation | Seed | Class | Violation |
|---|---|---|---|
$head_rows

\`$coverage\`

Every coverage oracle declared in the binary fired, the service's and client's own
\`reachable!\` sites included, except those \`sim.sh\` lists as out of this testbed's reach
($(paste -sd' ' <<<"$out_of_reach" | sed -E 's/([a-z-]+)/`\1`/g'); DECISIONS.md, 6.1 integration).
The fault sites are listed by the runs they fired in:

| Site | Kind | Generations |
|---|---|---|
$site_rows

## The planted bug, found and minimized

Turso's commit-time revision check skipping the target domain, planted in a build of its own:

\`\`\`diff
$planted_diff
\`\`\`

The same campaign over the planted build:

| Generation | Seed | Class | Violation |
|---|---|---|---|
$planted_rows

Generation $failing reduced to the knobs it needs (\`cargo patina minimize --generation $failing
--no-trace-phase\`): $needs

\`\`\`
\$ $shown_repro
exit status: $repro_status
$(grep -E '^PATINA_VERDICT' <<<"$repro_log" | cut -c1-240)
$(grep -E '^MULTIPLAYER_VIOLATION' <<<"$repro_log" | head -4 | cut -c1-240)
... ($(grep -cE '^MULTIPLAYER_VIOLATION' <<<"$repro_log") violations in all)
\`\`\`

A commit waits before it begins while another that loaded the same revision lands; with the
check skipped it lands too, at the same revision. Rewritten as a deterministic service test
with the commit held at a gate (\`crates/service/tests/in_flight.rs\`), failing with the bug
planted and passing with it removed:

\`\`\`
\$ cargo test -p cairn-service --test in_flight -- $race_test   # planted
$(grep -A1 'expected a stale answer' <<<"$test_log" | paste -sd' ' | tr -s ' ' | cut -c1-240)
$(grep -E '^test result' <<<"$test_log")
\$ cargo test -p cairn-service --test in_flight -- $race_test   # removed
$(grep -E '^test result' <<<"$unplanted_log" | cut -c1-240)
\`\`\`

## The product finding (fixed)

The first campaigns found an H5 bug (DECISIONS.md, "a resubmission beside its own original in
flight is answered stale"): a patch resubmitted while its original was still committing was
answered stale, naming the revision in flight with nothing intervening, and its caller was
told it conflicted although it landed. It is fixed: a Turso commit takes its turn at the rows
it writes and at its patch id, and the client never rebases backward. Its two cases run in
\`crates/service/tests/in_flight.rs\` over both stores:

\`\`\`
\$ cargo test -p cairn-service --test in_flight
$(grep -E '^test result' <<<"$fixed_log" | cut -c1-240)
\`\`\`
EOF
} >"$readme"

if [ "$mismatches" -ne 0 ]; then
  echo "prove.sh: $mismatches outcome(s) differed from what was expected" >&2
  exit 1
fi
echo "prove.sh: wrote $readme"
