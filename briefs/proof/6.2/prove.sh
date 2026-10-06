#!/usr/bin/env bash
# Generates briefs/proof/6.2/README.md: the durability testbed (testbeds/durability) built
# shim-linked and run under patina. One seed fault-free, under a failed fsync, under a
# crash-restart, and under both, which loses an acknowledged commit (the store finding,
# DECISIONS.md); a crash-restart sample; a smoke campaign's generations and oracles; a
# crash sweep and the oracles it fired; the planted client bug passing fault-free and
# caught by a crash; and the finding's deterministic store test, ignored in the ladder,
# failing when run. Exits non-zero if any outcome differs from the one it expects.
#
# usage: briefs/proof/6.2/prove.sh
#
# Needs cargo-patina (mise install) and jq. Writes the README and the testbed's target
# directory.
# shellcheck disable=SC2016 # backticks in single quotes are Markdown code spans
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
readme=$repo/briefs/proof/6.2/README.md
export CARGO_TERM_COLOR=never
mismatches=0
mismatch() {
  mismatches=$((mismatches + 1))
  echo "prove.sh: MISMATCH: $*" >&2
}

cd "$repo/testbeds/durability"
cargo build --locked --quiet # creates the managed target directory
out=target/patina/proof
mkdir -p "$out"
bed=target/patina/cairn-durability
cargo patina build . --output "$bed" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  exit 1
}
cargo patina audit "$bed" >"$out/audit.log" 2>&1 || true
allow=$(grep -oE '^    (dlopen|dlclose)|symbol=simsimd_[a-z0-9_]+_sapphire' "$out/audit.log" |
  sed 's/^ *//; s/^symbol=//' | sort -u | paste -sd, -)

# run ARGS...: one run; prints its stderr (stdout is the guest's, unused).
# shellcheck disable=SC2069
run() { cargo patina run "$bed" --allow-unsupported-symbols "$allow" "$@" 2>&1 >/dev/null || true; }
# verdict LOG: the run's verdict kind and label.
verdict() { grep -oE '^PATINA_VERDICT .* kind=[a-z_]+ label=[a-z-]+' <<<"$1" | head -1 | sed -E 's/.*kind=([a-z_]+) label=/\1 /'; }
line() { grep -E "^$1 " <<<"$2" | head -1 | cut -d' ' -f2- || true; }

# 1. One seed under each fault.
faults_rows=()
fault_case() {
  local title=$1 expected=$2
  shift 2
  local log got
  log=$(run --seed 31 "$@")
  got=$(verdict "$log")
  [ "$got" = "$expected" ] || mismatch "$title: expected '$expected', got '$got'"
  local retry restart violation
  retry=$(line DURABILITY_RETRY "$log")
  restart=$(line DURABILITY_RESTART "$log")
  violation=$(line DURABILITY_VIOLATION "$log")
  faults_rows+=("| $title | ${*:-none} | $got | ${retry:--} | ${restart:--} | ${violation:--} |")
}
fault_case "fault-free" "pass durability-outcome"
fault_case "a failed fsync" "pass durability-outcome" --fs-error-permille 5
fault_case "a crash-restart" "pass durability-outcome" --fs-crash-at write:40
fault_case "both" "violation durability-acknowledged-commit-present" \
  --fs-error-permille 5 --fs-crash-at write:40

# 2. Crash-restarts after seven consecutive syncs (one step's worth), seed 1.
restart_rows=()
for at in 38 39 40 41 42 43 44; do
  log=$(run --seed 1 --fs-crash-at "sync:$at")
  got=$(verdict "$log")
  [ "$got" = "pass durability-outcome" ] || mismatch "crash at sync:$at: got '$got'"
  restart=$(line DURABILITY_RESTART "$log")
  restart_rows+=("| \`sync:$at\` | ${restart:-no step in flight} | $got |")
done

# 3. A smoke campaign.
rm -rf "$out/campaign"
cargo patina campaign "$bed" --gens 16 --buggify --faults --fault-scale-permille 30 --swarm \
  --allow-unmet-sometimes --allow-unsupported-symbols "$allow" --out-dir "$out/campaign" \
  --progress-every 0 >"$out/campaign.log" 2>&1 || mismatch "the smoke campaign failed"
grep -q '^generations=16 failures=0 ' "$out/campaign.log" || mismatch "a campaign generation failed"
campaign_summary=$(sed -n '/== campaign summary ==/,/^-- coverage/p' "$out/campaign.log" | sed '$d')
sites=$(jq -r '.sites[] | select(.label | startswith("durability-") or startswith("service-resubmission-answered")) |
  "| `\(.label)` | \(.kind) | \(.registered_gens) | \(.satisfied_gens) | \(.fires) |"' \
  "$out/campaign/sites.json")

# 4. A crash sweep (seed 1, byte tearing, every 11th write and sync).
sweep_runs=0
sweep_complete=0
fired=""
for kind in write sync; do
  for at in $(seq 2 11 145); do
    log=$(run --seed 1 --fs-crash-at "$kind:$at" --fs-torn-granularity byte)
    sweep_runs=$((sweep_runs + 1))
    grep -q '^DURABILITY_RESULT outcome=complete ' <<<"$log" && sweep_complete=$((sweep_complete + 1))
    fired+=$(grep -h '^PATINA_SDK_REPORT ' <<<"$log" | tr ' ' '\n' | grep -E '^site=[^|]+\|sometimes\|' |
      grep -E '\|s[1-9][0-9]*\|' | sed 's/^site=//; s/|.*//' || true)
    fired+=$'\n'
  done
done
[ "$sweep_runs" = "$sweep_complete" ] || mismatch "crash sweep: $sweep_complete of $sweep_runs completed"
fired_list=$(grep -v '^$' <<<"$fired" | sort | uniq -c | awk '{ printf "- `%s`: %s runs\n", $2, $1 }')

# 5. The planted client bug.
planted_free=$(verdict "$(run --seed 1 -- --bug ack-before-commit)")
planted_crash_log=$(run --seed 1 --fs-crash-at sync:40 -- --bug ack-before-commit)
planted_crash=$(verdict "$planted_crash_log")
[ "$planted_free" = "pass durability-outcome" ] || mismatch "planted, fault-free: $planted_free"
[ "$planted_crash" = "violation durability-acknowledged-commit-present" ] || mismatch "planted, crash: $planted_crash"

# 6. The finding's store test, ignored in the ladder, run on purpose.
cd "$repo"
test_log=$(cargo test --locked -p cairn-store-turso --all-features --test conformance -- \
  conformance::a_commit_whose_log_sync_fails_leaves_nothing_visible --include-ignored 2>&1 || true)
grep -q 'test result: FAILED. 0 passed; 1 failed' <<<"$test_log" || mismatch "the finding's test did not fail"
test_failure=$(grep -A3 'panicked at' <<<"$test_log" | sed -E 's/ \([0-9]+\)//; s/at .*(crates\/)/at \1/')

{
  echo '# Proof: brief 6.2, durability testbed'
  echo
  echo 'Generated by `briefs/proof/6.2/prove.sh`; do not edit by hand. Every run is the'
  echo 'testbed (`testbeds/durability`) built with `cargo patina build` and run under the patina'
  echo 'native shim: the service on the real Turso store in a simulated filesystem, one client'
  echo 'submitting a 24-step seeded plan and keeping a durable ledger of what was acknowledged.'
  echo 'After every restart and at the end the run checks that no acknowledged commit is'
  echo 'missing, that each commit is whole or absent, that the log holds only what was'
  echo 'submitted, and that the state loaded equals the engine'"'"'s replay of the log.'
  echo
  echo '## An injected fault changes the outcome'
  echo
  echo 'Seed 31. A failed fsync alone is retried; a crash alone is recovered; both together lose'
  echo 'an acknowledged commit. Step 2'"'"'s log fsync fails, the store answers `Failed`, the client'
  echo 'resubmits, and the resubmission is answered from a receipt that never reached the disk;'
  echo 'the crash before step 3'"'"'s log sync then takes the commit away (DECISIONS.md, 6.2).'
  echo
  echo '| Run | Faults | Verdict | Retried write | Restart | Violation |'
  echo '|---|---|---|---|---|---|'
  printf '%s\n' "${faults_rows[@]}"
  echo
  echo '## Crash-restart at each commit point'
  echo
  echo 'Seed 1, crashing after each of seven consecutive syncs, one step'"'"'s worth: the'
  echo 'ledger'"'"'s intent, the store'"'"'s four commit points (recorded in the ledger by its fault'
  echo 'hook), the log, and the ack. The restarted incarnation audits the store, resubmits the'
  echo 'step in flight, and finishes.'
  echo
  echo '| Crash | Restart | Verdict |'
  echo '|---|---|---|'
  printf '%s\n' "${restart_rows[@]}"
  echo
  echo '## Smoke campaign'
  echo
  echo '`cargo patina campaign --gens 16 --buggify --faults --fault-scale-permille 30 --swarm`:'
  echo
  echo '```'
  echo "$campaign_summary"
  echo '```'
  echo
  echo '| Site | Kind | Generations reached | Generations satisfied | Fires |'
  echo '|---|---|---|---|---|'
  echo "$sites"
  echo
  echo 'The crash oracles cannot fire in a campaign, which draws no crash; the sweep below'
  echo 'gates them (README, Findings).'
  echo
  echo '## Crash sweep'
  echo
  echo "Seed 1, byte-granular tearing, crash after every 11th write and sync: $sweep_complete of"
  echo "$sweep_runs crash-restarts recovered and completed with every invariant holding. The"
  echo 'restart oracles that fired, by number of runs:'
  echo
  echo "$fired_list"
  echo
  echo '## Planted bug'
  echo
  echo 'The client bug `--bug ack-before-commit` records each step as acknowledged before it'
  echo 'submits it. Seed 1 fault-free: `'"$planted_free"'`. With a crash after the 40th sync:'
  echo '`'"$planted_crash"'`:'
  echo
  echo '```'
  grep '^DURABILITY_VIOLATION' <<<"$planted_crash_log"
  echo '```'
  echo
  echo '## The finding as a store test'
  echo
  echo '`crates/store-turso/tests/conformance/log_sync.rs` wraps the platform I/O to fail one'
  echo 'log fsync. It is ignored in the ladder until the fix; run on purpose it fails:'
  echo
  echo '```'
  echo "$test_failure"
  echo '```'
} >"$readme"

[ "$mismatches" -eq 0 ] || {
  echo "prove.sh: $mismatches outcome(s) differ from what is expected; see $readme" >&2
  exit 1
}
echo "prove.sh: wrote $readme"
