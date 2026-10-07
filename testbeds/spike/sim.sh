#!/usr/bin/env bash
# The spike's legs of `mise run sim` (brief 1.3; PRACTICES, Simulation with patina). Not part
# of `mise run check`. Each leg fails loudly; none skips.
#
#   1. code: rustfmt, clippy with warnings denied, the unit tests;
#   2. build shim-linked, and `cargo patina audit` with no allowance;
#   3. determinism: one seed recorded twice is byte-identical, and its replay reports the
#      same verdict;
#   4. smoke campaign: buggify and PCT schedules over 16 generations, every `reachable!`
#      site fired;
#   5. faulted sweep: 8 seeds with dropped, refused, reset, and delayed connections; every
#      run passes, every run retried, and no fault knob was inert;
#   6. known patina gaps (README, Patina gap report): each must still reproduce exactly as
#      recorded. One that stops reproducing fails this leg, so a patina bump that fixes a gap
#      gets its README section and its file in decisions/ updated instead of going stale.
#
# The current-thread runtime with `--tick-ms 1` is the configuration that works under the
# shim; the gap legs run the ones that do not.
set -euo pipefail
cd "$(dirname "$0")"

say() { printf 'sim: spike: %s\n' "$*"; }
fail() {
  say "FAILED: $*" >&2
  exit 1
}
command -v cargo-patina >/dev/null || fail "cargo-patina is missing: run mise install"

# Builds the artifact first, so `target` (a managed symlink on some hosts) exists.
cargo fmt --check || fail "rustfmt: run cargo fmt in testbeds/spike"
cargo clippy --locked --all-targets -- -D warnings || fail "clippy reported errors"
cargo test --locked || fail "unit tests failed"
say "leg 1 passed: code"

out=target/patina
mkdir -p "$out"
spike=$out/cairn-spike
cargo patina build . --output "$spike" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  fail "cargo patina build"
}
cargo patina audit "$spike" >"$out/audit.log" 2>&1 || {
  cat "$out/audit.log" >&2
  fail "cargo patina audit found unmodeled effects"
}
say "leg 2 passed: built and audited with no allowance"

# The verdict line a run reported, from its stderr file.
verdict_of() { grep '^PATINA_VERDICT ' "$1" || true; }

for seed in 1 2; do
  for repeat in a b; do
    cargo patina run "$spike" --seed "$seed" --record "$out/seed-$seed-$repeat.patina" \
      -- --tick-ms 1 >/dev/null 2>"$out/seed-$seed-$repeat.err" ||
      fail "seed $seed did not pass: $out/seed-$seed-$repeat.err"
  done
  cmp -s "$out/seed-$seed-a.patina" "$out/seed-$seed-b.patina" ||
    fail "seed $seed recorded twice gave different traces"
  cargo patina replay "$spike" "$out/seed-$seed-a.patina" >/dev/null 2>"$out/seed-$seed-replay.err" ||
    fail "seed $seed replay failed: $out/seed-$seed-replay.err"
  [ "$(verdict_of "$out/seed-$seed-a.err")" = "$(verdict_of "$out/seed-$seed-replay.err")" ] ||
    fail "seed $seed replay reported a different verdict"
done
say "leg 3 passed: repeats byte-identical, replays reproduce the verdict"

rm -rf "$out/campaign"
cargo patina campaign "$spike" --gens 16 --buggify --sched-pct --out-dir "$out/campaign" \
  --progress-every 0 -- --tick-ms 1 >"$out/campaign.log" 2>&1 || {
  sed -n '/campaign summary/,$p' "$out/campaign.log" >&2
  fail "smoke campaign"
}
grep 'PATINA_CAMPAIGN_COVERAGE' "$out/campaign.log"
say "leg 4 passed: smoke campaign"

seeds=8
cargo patina explore run "$spike" --seeds "$seeds" \
  --net-drop-permille 100 --net-latency-nanos 1000000 --net-jitter-nanos 0..2000000 \
  --net-connect-refuse-permille 100 --net-reset-permille 100 \
  -- --tick-ms 1 >"$out/sweep.log" 2>&1 || {
  grep -E '^(PATINA_EXPLORE|SPIKE_)' "$out/sweep.log" >&2 || true
  fail "faulted sweep"
}
passed=$(grep -c '^SPIKE_RESULT ' "$out/sweep.log" || true)
retried=$(grep '^SPIKE_RESULT ' "$out/sweep.log" | grep -cv ' retries=0 ' || true)
applied=$(grep -c '^PATINA_NET_FAULT_REPORT .* vacuous=0' "$out/sweep.log" || true)
[ "$passed" -eq "$seeds" ] || fail "faulted sweep: $passed of $seeds runs passed"
[ "$retried" -eq "$seeds" ] || fail "faulted sweep: only $retried of $seeds runs retried"
[ "$applied" -eq "$seeds" ] || fail "faulted sweep: a net fault knob was inert"
say "leg 5 passed: faulted sweep, $seeds seeds, every run retried"

# gap NAME PATTERN COMMAND...: COMMAND must fail and print PATTERN.
gap() {
  local name=$1 pattern=$2 status=0
  shift 2
  "$@" >"$out/gap-$name.log" 2>&1 || status=$?
  if [ "$status" -eq 0 ] || ! grep -Eq "$pattern" "$out/gap-$name.log"; then
    fail "patina gap '$name' no longer reproduces (exit $status): update testbeds/spike/README.md and its file in decisions/"
  fi
  say "gap $name still reproduces: $(grep -Eo "$pattern" "$out/gap-$name.log" | head -1)"
}
gap multi-thread 'shim fatal: [a-z_]+' \
  cargo patina explore run "$spike" --seeds 40 -- --flavor multi-thread --tick-ms 1
gap thread-per-client 'shim fatal: [a-z_]+' \
  cargo patina explore run "$spike" --seeds 40 -- --flavor thread-per-client --tick-ms 1
gap delayed-delivery 'SPIKE_FAILURE .* timed out' \
  cargo patina run "$spike" --seed 1 --net-latency-nanos 1000000
rm -rf "$out/campaign-faults"
gap campaign-faults-vacuous 'class VACUOUS_NET_FAULT' \
  cargo patina campaign "$spike" --gens 4 --faults --buggify --out-dir "$out/campaign-faults" \
  --progress-every 0 -- --tick-ms 1
say "leg 6 passed: known gaps reproduce as recorded"
