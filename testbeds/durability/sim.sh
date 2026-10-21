#!/usr/bin/env bash
# The durability testbed's legs of `mise run sim` (brief 6.2; PRACTICES, Simulation with
# patina). Not part of `mise run check`. Each leg fails loudly; none skips.
#
#   1. code: the unit tests (rustfmt and clippy are the check ladder's lint step);
#   2. build shim-linked, and `cargo patina audit` finds no unsupported symbol;
#   3. determinism: a crash-restart run recorded twice is byte-identical, and its replay
#      reports the same verdict;
#   4. smoke campaign: buggify, dampened fs errors and short I/O, and swarm over 128
#      generations; every generation passes, and every declared oracle fired except the
#      ones named below as out of this testbed's reach;
#   5. crash sweep: crash-restart at every 11th write and sync, with whole-block and
#      byte-granular tearing, over two seeds; every run restarts and completes with the
#      invariants holding, and every crash oracle fires somewhere in the sweep;
#   6. crash under errors: the sweep's crash points with fs errors too; no run breaks an
#      invariant (a store that does not open on a failing disk is an honest outcome; a
#      store that fails closed ends the run `unavailable`).
#
# The known patina gaps (README, Findings) are not checked here: they exercise that pinned
# dependency, not Cairn, and are recorded in decisions/
# (decisions/2026-10-17-simulation-runs-no-known-gap-checks.md). No check here depends on
# one seed's outcome (decisions/2026-10-16-simulation-checks-pin-no-seed.md).
set -euo pipefail
cd "$(dirname "$0")"

say() { printf 'sim: durability: %s\n' "$*"; }
fail() {
  say "FAILED: $*" >&2
  exit 1
}
command -v cargo-patina >/dev/null || fail "cargo-patina is missing: run mise install"

# Builds the artifact first, so `target` (a managed symlink on some hosts) exists. rustfmt and
# clippy are the check ladder's lint step (PRACTICES, The validation ladder), not repeated here.
cargo test --locked || fail "unit tests failed"
say "leg 1 passed: code"

out=target/patina
mkdir -p "$out"
bed=$out/cairn-durability
cargo patina build . --output "$bed" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  fail "cargo patina build"
}
cargo patina audit "$bed" >"$out/audit.log" 2>&1 || {
  cat "$out/audit.log" >&2
  fail "cargo patina audit"
}
say "leg 2 passed: built and audited, no unsupported symbol"

# run ARGS...: one run of the testbed under patina.
run() { cargo patina run "$bed" "$@"; }
verdict_of() { grep '^PATINA_VERDICT ' "$1" || true; }

for repeat in a b; do
  run --seed 3 --fs-crash-at sync:40 --fs-torn-granularity byte \
    --record "$out/crash-$repeat.patina" >/dev/null 2>"$out/crash-$repeat.err" ||
    fail "the recorded crash run did not pass: $out/crash-$repeat.err"
done
cmp -s "$out/crash-a.patina" "$out/crash-b.patina" ||
  fail "a crash-restart run recorded twice gave different traces"
cargo patina replay "$bed" "$out/crash-a.patina" \
  >/dev/null 2>"$out/crash-replay.err" || fail "replay failed: $out/crash-replay.err"
[ "$(verdict_of "$out/crash-a.err")" = "$(verdict_of "$out/crash-replay.err")" ] ||
  fail "the replay reported a different verdict"
say "leg 3 passed: a crash-restart run repeats byte-identically and replays its verdict"

# Declared oracles this testbed cannot reach, by label. The campaign's own gate is waived
# for these alone: any other oracle that never fires fails leg 4, and one of these firing
# is reported so it can leave the list.
out_of_reach=(
  # Two writers in flight on one domain (the multiplayer testbed's ground): this testbed
  # has one client, which moves on only once a step is acknowledged.
  service-duplicate-original-committed-meanwhile
  service-patch-lost-at-commit
  service-resubmission-raced-original
  service-stale-patch-completed-with-intervening
  # Derived reads, proposals, and route-file imports: the plan writes domain patches only.
  service-derived-read-answered-from-memo
  service-import-resubmission-rebuilt-from-receipt
  service-proposal-apply-answered-from-receipt
  service-proposal-create-resubmitted
  service-proposal-draft-answered-from-receipt
  service-proposal-draft-resubmitted
  service-proposal-patch-answered-from-receipt
  service-proposal-refreshed-against-destination
)
# The rarest oracle in reach (a failed write retried) fired in 7 of 128 generations
# measured, so a generation misses it with probability 121/128 and 128 generations miss it
# with (121/128)^128, about 0.07%: less than once in 1,000 nights.
gens=128
rm -rf "$out/campaign"
cargo patina campaign "$bed" --gens "$gens" --buggify --faults --fault-scale-permille 100 --swarm \
  --allow-unmet-sometimes --out-dir "$out/campaign" \
  --progress-every 0 >"$out/campaign.log" 2>&1 || {
  sed -n '/campaign summary/,$p' "$out/campaign.log" >&2
  fail "smoke campaign"
}
grep -q "^generations=$gens failures=0 " "$out/campaign.log" || fail "smoke campaign: a generation failed"
# The coverage gate, by hand: --allow-unmet-sometimes waives every site, so this requires
# every site but the ones named above.
unmet=$(jq -r '.sites[] | select((.kind == "sometimes" or .kind == "reachable") and .satisfied_gens == 0) | .label' \
  "$out/campaign/sites.json" | sort)
for label in $unmet; do
  printf '%s\n' "${out_of_reach[@]}" | grep -qx -- "$label" ||
    fail "smoke campaign: the oracle $label never fired"
done
for label in "${out_of_reach[@]}"; do
  grep -qx -- "$label" <<<"$unmet" || say "note: $label fired: take it out of out_of_reach"
done
inert=$(jq -r '.sites[] | select(.kind == "fault" and .fires == 0) | .label' "$out/campaign/sites.json")
[ -z "$inert" ] || fail "smoke campaign: buggify sites never fired: $inert"
say "leg 4 passed: smoke campaign, $gens generations, $(jq '[.sites[] | select(.satisfied_gens > 0)] | length' \
  "$out/campaign/sites.json") oracles fired, every one in reach"

# sweep NAME [FLAG...]: crash-restart at every 11th write and sync from the 2nd to the
# 145th, with both tearing granularities, over seeds 1 and 2. A step makes one write and
# one sync per ledger entry and commit point plus the log's (seven of each with the store's
# four commit points); 11 is prime to that, so every point of a step is hit. Writes one line
# per run to $out/NAME.runs.
# shellcheck disable=SC2016 # the xargs script is expanded by its own shell
sweep() {
  local name=$1
  shift
  rm -f "$out/$name".*.err
  extra="$*"
  export bed out name extra
  for seed in 1 2; do
    for granularity in block byte; do
      for kind in write sync; do
        for at in $(seq 2 11 145); do
          printf '%s %s %s %s\n' "$seed" "$granularity" "$kind" "$at"
        done
      done
    done
  done | xargs -P 4 -L 1 bash -c '
    seed=$1 granularity=$2 kind=$3 at=$4
    err="$out/$name.$seed.$granularity.$kind.$at.err"
    status=0
    cargo patina run "$bed" --seed "$seed" \
      --fs-crash-at "$kind:$at" --fs-torn-granularity "$granularity" $extra >/dev/null 2>"$err" ||
      status=$?
    echo "$status $err"
  ' _ >"$out/$name.runs"
}

# satisfied FILES...: the labels of sometimes! sites satisfied in any of the runs' reports.
satisfied() {
  cat "$@" | grep -h '^PATINA_SDK_REPORT ' | tr ' ' '\n' | grep -E '^site=[^|]+\|sometimes\|' |
    grep -E '\|s[1-9][0-9]*\|' | sed 's/^site=//; s/|.*//' | sort -u
}

sweep crash
runs=$(wc -l <"$out/crash.runs")
bad=$(awk '$1 != 0' "$out/crash.runs")
[ -z "$bad" ] || fail "crash sweep: runs failed: $bad"
mapfile -t crash_logs < <(awk '{print $2}' "$out/crash.runs")
for err in "${crash_logs[@]}"; do
  grep -q '^PATINA_FS_CRASH_RESTART .*result=restarted' "$err" || fail "crash sweep: no restart in $err"
  grep -q '^DURABILITY_RESULT outcome=complete ' "$err" || fail "crash sweep: the store did not open or finish: $err"
done
# Byte tearing leaves part of a frame at the end of SQLite's write-ahead log, which the open
# discards, so the torn-tail oracle is required too.
crash_oracles='durability-crash-between-state-and-events
durability-crash-interrupted-commit-landed
durability-crash-interrupted-commit-lost
durability-restart-recovered-commits
durability-torn-log-tail-discarded'
fired=$(satisfied "${crash_logs[@]}")
missing=$(comm -23 <(sort <<<"$crash_oracles") <(printf '%s\n' "$fired"))
[ -z "$missing" ] || fail "crash sweep: oracles never fired: $missing"
say "leg 5 passed: crash sweep, $runs crash-restarts, every one recovered; fired: $(paste -sd, <<<"$fired")"

# No run breaks an invariant (README, Findings: a commit that fails at its fsync fails the
# store closed, which ends the run). A store that does not open on a failing disk is an
# honest outcome.
sweep crash-errors --fs-error-permille 5
while read -r status err; do
  if grep -q '^PATINA_VERDICT .*kind=violation' "$err"; then
    fail "crash under errors: an invariant broke: $err"
  elif [ "$status" != 0 ]; then
    # A store that did not open ends the run before the crash point it was given.
    if ! grep -q '^DURABILITY_RESULT outcome=unavailable ' "$err" ||
      ! grep -q 'PATINA_FS_CRASH_SELECTOR_UNREACHED' "$err"; then
      fail "crash under errors: a run failed: $err"
    fi
  fi
done <"$out/crash-errors.runs"
complete=$(cat $(awk '{print $2}' "$out/crash-errors.runs") | grep -c '^DURABILITY_RESULT outcome=complete ' || true)
say "leg 6 passed: crash under errors, $complete of $(wc -l <"$out/crash-errors.runs") runs completed, no invariant broke"
