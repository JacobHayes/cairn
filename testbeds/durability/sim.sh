#!/usr/bin/env bash
# The durability testbed's legs of `mise run sim` (brief 6.2; PRACTICES, Simulation with
# patina). Not part of `mise run check`. Each leg fails loudly; none skips.
#
#   1. code: rustfmt, clippy with warnings denied, the unit tests;
#   2. build shim-linked, and `cargo patina audit`: the only unsupported symbols are the
#      ones Turso links and Cairn never reaches (README, Audit), allowed by name;
#   3. determinism: a crash-restart run recorded twice is byte-identical, and its replay
#      reports the same verdict;
#   4. smoke campaign: buggify, dampened fs errors and short I/O, and swarm over 32
#      generations; every generation passes, and every declared oracle fired except the
#      ones named below as out of this testbed's reach;
#   5. crash sweep: crash-restart at every 11th write and sync, with whole-block and
#      byte-granular tearing, over two seeds; every run restarts and completes with the
#      invariants holding, and every crash oracle fires somewhere in the sweep;
#   6. crash under errors: the sweep's crash points with fs errors too; no run breaks an
#      invariant but through the one known store finding, which must still reproduce (a
#      store that does not open on a failing disk is an honest outcome);
#   7. known Turso and patina gaps (README, Findings): each must still reproduce exactly as
#      recorded. One that stops reproducing fails this leg, so a Turso or patina bump that
#      fixes it gets its README section and DECISIONS.md entry updated instead of going
#      stale.
set -euo pipefail
cd "$(dirname "$0")"

say() { printf 'sim: durability: %s\n' "$*"; }
fail() {
  say "FAILED: $*" >&2
  exit 1
}
command -v cargo-patina >/dev/null || fail "cargo-patina is missing: run mise install"

# Builds the artifact first, so `target` (a managed symlink on some hosts) exists.
cargo fmt --check || fail "rustfmt: run cargo fmt in testbeds/durability"
cargo clippy --locked --all-targets -- -D warnings || fail "clippy reported errors"
cargo test --locked || fail "unit tests failed"
say "leg 1 passed: code"

out=target/patina
mkdir -p "$out"
bed=$out/cairn-durability
cargo patina build . --output "$bed" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  fail "cargo patina build"
}
# The audit exits nonzero while anything is unsupported; what it lists is checked below.
cargo patina audit "$bed" >"$out/audit.log" 2>&1 || true
# Every unsupported finding is extension loading (dlopen, dlclose) or one of simsimd's
# AVX-512 FP16 kernels, which Turso links and Cairn never calls, or rustix's raw syscall,
# which a run traps through syscall-user dispatch and needs no allowance.
unsupported=$(sed -n '/^unsupported native imports:/,/^[^ ]/p' "$out/audit.log" | grep -E '^    ' || true)
unexpected=$(grep -Ev '^    (dlopen|dlclose) |symbol=simsimd_[a-z0-9_]+_sapphire |\(direct-syscall\) .*rustix' \
  <<<"$unsupported" || true)
[ -z "$unexpected" ] || {
  printf '%s\n' "$unexpected" >&2
  fail "cargo patina audit found unsupported symbols beyond Turso's known ones"
}
allow=$(grep -oE '^    (dlopen|dlclose)|symbol=simsimd_[a-z0-9_]+_sapphire' <<<"$unsupported" |
  sed 's/^ *//; s/^symbol=//' | sort -u | paste -sd, -)
[ -n "$allow" ] || fail "the audit listed none of Turso's known symbols: update this leg and the README"
say "leg 2 passed: built and audited, allowing $(tr ',' '\n' <<<"$allow" | wc -l) symbols Turso links"

# run ARGS...: one run of the testbed under patina, with the allowance.
run() { cargo patina run "$bed" --allow-unsupported-symbols "$allow" "$@"; }
verdict_of() { grep '^PATINA_VERDICT ' "$1" || true; }

for repeat in a b; do
  run --seed 3 --fs-crash-at sync:40 --fs-torn-granularity byte \
    --record "$out/crash-$repeat.patina" >/dev/null 2>"$out/crash-$repeat.err" ||
    fail "the recorded crash run did not pass: $out/crash-$repeat.err"
done
cmp -s "$out/crash-a.patina" "$out/crash-b.patina" ||
  fail "a crash-restart run recorded twice gave different traces"
cargo patina replay "$bed" "$out/crash-a.patina" --allow-unsupported-symbols "$allow" \
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
  # A read of the revisions failing in the instant after a commit that creates an entity in
  # passing: injected errors land there too rarely for a smoke campaign.
  service-deployment-revision-unread
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
rm -rf "$out/campaign"
cargo patina campaign "$bed" --gens 32 --buggify --faults --fault-scale-permille 30 --swarm \
  --allow-unmet-sometimes --allow-unsupported-symbols "$allow" --out-dir "$out/campaign" \
  --progress-every 0 >"$out/campaign.log" 2>&1 || {
  sed -n '/campaign summary/,$p' "$out/campaign.log" >&2
  fail "smoke campaign"
}
grep -q '^generations=32 failures=0 ' "$out/campaign.log" || fail "smoke campaign: a generation failed"
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
say "leg 4 passed: smoke campaign, 32 generations, $(jq '[.sites[] | select(.satisfied_gens > 0)] | length' \
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
  export bed allow out name extra
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
    cargo patina run "$bed" --allow-unsupported-symbols "$allow" --seed "$seed" \
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
# The torn-tail oracle is not required: at the pinned patina no crash leaves part of a Turso
# log frame on disk (README, Findings), which leg 7 checks still holds.
crash_oracles='durability-crash-between-state-and-events
durability-crash-interrupted-commit-landed
durability-crash-interrupted-commit-lost
durability-restart-recovered-commits'
fired=$(satisfied "${crash_logs[@]}")
missing=$(comm -23 <(sort <<<"$crash_oracles") <(printf '%s\n' "$fired"))
[ -z "$missing" ] || fail "crash sweep: oracles never fired: $missing"
say "leg 5 passed: crash sweep, $runs crash-restarts, every one recovered; fired: $(paste -sd, <<<"$fired")"

# A run breaks an invariant here only through the known store finding (README, Findings:
# an acknowledged commit lost after a failed log fsync): the commit's log sync failed, the
# retry was answered from a receipt that was never made durable, and the crash lost it.
# Any other violation, or another failure, fails the leg; so does the finding no longer
# reproducing, which means the store was fixed and this leg and the README are stale.
sweep crash-errors --fs-error-permille 5
known=0
while read -r status err; do
  if grep -q '^PATINA_VERDICT .*kind=violation' "$err"; then
    labels=$(grep -o '^PATINA_VERDICT .*kind=violation label=[^ ]*' "$err" | sed 's/.*label=//' | sort -u)
    if [ "$labels" != durability-acknowledged-commit-present ] ||
      ! grep -q '^DURABILITY_RETRY .*I/O error (sync)' "$err"; then
      fail "crash under errors: an invariant broke other than by the known finding: $err"
    fi
    known=$((known + 1))
  elif [ "$status" != 0 ]; then
    # A store that did not open ends the run before the crash point it was given.
    if ! grep -q '^DURABILITY_RESULT outcome=unavailable ' "$err" ||
      ! grep -q 'PATINA_FS_CRASH_SELECTOR_UNREACHED' "$err"; then
      fail "crash under errors: a run failed: $err"
    fi
  fi
done <"$out/crash-errors.runs"
# The finding, minimized: step 2's log fsync fails, the resubmission is answered from the
# receipt, and the crash lands before step 3's log sync.
run --seed 31 --fs-error-permille 5 --fs-crash-at write:40 >/dev/null 2>"$out/finding.err" || true
if ! grep -q '^PATINA_VERDICT .*kind=violation label=durability-acknowledged-commit-present' "$out/finding.err" ||
  ! grep -q '^DURABILITY_RETRY step=2 .*I/O error (sync)' "$out/finding.err"; then
  fail "store finding 'lost-unsynced-ack' no longer reproduces ($out/finding.err): update README.md, DECISIONS.md, this leg, and un-ignore its test in crates/store-turso/tests/conformance.rs"
fi
mapfile -t error_logs < <(awk '{print $2}' "$out/crash-errors.runs")
complete=$(grep -l '^DURABILITY_RESULT outcome=complete ' "${error_logs[@]}" | wc -l)
say "leg 6 passed: crash under errors, $complete of $(wc -l <"$out/crash-errors.runs") runs completed; $known lost an acknowledged commit after a failed log fsync, as the pinned run does (known finding); no other violation"

# gap NAME PATTERN ARGS...: a run with ARGS must still print PATINA.
gap() {
  local name=$1 pattern=$2
  shift 2
  run "$@" >"$out/gap-$name.log" 2>&1 || true
  grep -Eq "$pattern" "$out/gap-$name.log" ||
    fail "Turso gap '$name' no longer reproduces: update testbeds/durability/README.md and DECISIONS.md"
  say "gap $name still reproduces: $(grep -Eo "$pattern" "$out/gap-$name.log" | head -1)"
}
gap open-size-panic 'turso panicked: failed to get file size' --seed 23 --fs-error-permille 50
gap open-short-read 'Logical log short read: expected [0-9]+, got [0-9]+' --seed 3 --fs-short-permille 200
gap reopen-page-cache-panic 'Attempted to insert different page with same key' \
  --seed 6 --fs-error-permille 20 -- --open-attempts 64
torn=$(grep -l '^DURABILITY_RESTART .*torn_tail=true' "${crash_logs[@]}" || true)
[ -z "$torn" ] || fail "patina gap 'no-partial-log-frame' no longer reproduces ($torn): require durability-torn-log-tail-discarded in leg 5 and update README.md and DECISIONS.md"
say "gap no-partial-log-frame still reproduces: no crash left part of a Turso log frame"
say "leg 7 passed: known Turso and patina gaps reproduce as recorded"
