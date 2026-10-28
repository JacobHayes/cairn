#!/usr/bin/env bash
# The multiplayer testbed's legs of `mise run sim` (brief 6.1; PRACTICES, Simulation with
# patina). Not part of `mise run check`. Each leg fails loudly; none skips.
#
#   1. code: the unit tests (rustfmt and clippy are the check ladder's lint step);
#   2. build shim-linked; `cargo patina audit` finds no unsupported symbol;
#   3. determinism: one seed recorded twice is byte-identical, and its replay reports the
#      same verdict;
#   4. smoke campaign: buggify and PCT schedules over 16 generations, every `sometimes!`
#      and `reachable!` site fired but those out of reach below, and every generation
#      passes;
#   5. faulted sweep: 8 seeds with dropped, refused, reset, and delayed connections; every
#      run passes, every run resent a request, some patch was answered from its receipt,
#      and no fault knob was inert;
#
# The known patina gaps (README, Patina gap report) are not checked here: they exercise the
# pinned patina, not Cairn, and are recorded in decisions/
# (decisions/2026-10-17-simulation-runs-no-known-gap-checks.md).
#
# The testbed serves on a multi-thread runtime (decisions/2026-10-18-the-multiplayer-testbed-runs-a-multi-thread-runtime.md).
set -euo pipefail
cd "$(dirname "$0")"

say() { printf 'sim: multiplayer: %s\n' "$*"; }
fail() {
  say "FAILED: $*" >&2
  exit 1
}
command -v cargo-patina >/dev/null || fail "cargo-patina is missing: run mise install"

# Coverage oracles in the binary that this testbed cannot reach, by label (DECISIONS.md,
# 6.1 integration). The campaign's own gate is waived for these alone: any other oracle that
# never fires fails leg 4, and one of these firing is reported so it can leave the list.
out_of_reach=(
  # 4.8's derived-read memo: the views refetch the journey document, never a derived read.
  service-derived-read-answered-from-memo
  # 4.8's and 4.9's proposal and route-import paths: the testbed drafts no proposal and
  # imports no route file.
  service-import-resubmission-rebuilt-from-receipt
  service-proposal-apply-answered-from-receipt
  service-proposal-create-resubmitted
  service-proposal-draft-answered-from-receipt
  service-proposal-draft-resubmitted
  service-proposal-patch-answered-from-receipt
  service-proposal-refreshed-against-destination
  # 4.4's assistant: its tool loop, turn limits, and write wrapper. The testbed never drives
  # the assistant, so no turn runs.
  assistant-structural-write-proposed
  assistant-direct-write-applied
  assistant-iteration-limit-reached
  assistant-provider-call-timed-out
  assistant-tool-call-past-deadline
  assistant-turn-overflow-proposed
)

# Builds the artifact first, so `target` (a managed symlink on some hosts) exists. rustfmt and
# clippy are the check ladder's lint step (PRACTICES, The validation ladder), not repeated here.
cargo test --locked || fail "unit tests failed"
say "leg 1 passed: code"

out=target/patina
mkdir -p "$out"
testbed=$out/cairn-multiplayer
cargo patina build . --output "$testbed" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  fail "cargo patina build"
}
cargo patina audit "$testbed" >"$out/audit.log" 2>&1 || {
  cat "$out/audit.log" >&2
  fail "cargo patina audit"
}
say "leg 2 passed: built; the audit finds no unsupported symbol"

# The verdict line a run reported, from its stderr file.
verdict_of() { grep '^PATINA_VERDICT ' "$1" || true; }

for seed in 1 2; do
  for repeat in a b; do
    cargo patina run "$testbed" --seed "$seed" --record "$out/seed-$seed-$repeat.patina" \
      >/dev/null 2>"$out/seed-$seed-$repeat.err" ||
      fail "seed $seed did not pass: $out/seed-$seed-$repeat.err"
  done
  cmp -s "$out/seed-$seed-a.patina" "$out/seed-$seed-b.patina" ||
    fail "seed $seed recorded twice gave different traces"
  cargo patina replay "$testbed" "$out/seed-$seed-a.patina" \
    >/dev/null 2>"$out/seed-$seed-replay.err" || fail "seed $seed replay failed: $out/seed-$seed-replay.err"
  [ "$(verdict_of "$out/seed-$seed-a.err")" = "$(verdict_of "$out/seed-$seed-replay.err")" ] ||
    fail "seed $seed replay reported a different verdict"
done
say "leg 3 passed: repeats byte-identical, replays reproduce the verdict"

rm -rf "$out/campaign"
status=0
cargo patina campaign "$testbed" --gens 16 --buggify --sched-pct --out-dir "$out/campaign" \
  --progress-every 0 --allow-unmet-sometimes \
  >"$out/campaign.log" 2>&1 || status=$?
summary=$(sed -n '/campaign summary/,$p' "$out/campaign.log")
[ -n "$summary" ] || {
  tail -20 "$out/campaign.log" >&2
  fail "smoke campaign did not finish (exit $status)"
}
grep -q 'PATINA_CAMPAIGN_COVERAGE ' <<<"$summary" || fail "smoke campaign: no coverage summary"
unmet=$(grep -oE "^  UNMET [a-z]+ '[^']+'" <<<"$summary" | sed -E "s/.*'(.*)'/\1/" | sort -u)
for label in $unmet; do
  printf '%s\n' "${out_of_reach[@]}" | grep -qx -- "$label" ||
    fail "smoke campaign: the oracle $label never fired"
done
for label in "${out_of_reach[@]}"; do
  grep -qx -- "$label" <<<"$unmet" || say "note: $label fired: take it out of out_of_reach"
done
grep -q '^generations=16 failures=0 ' "$out/campaign.log" || {
  grep -E '^  (class|signature)' <<<"$summary" >&2
  fail "smoke campaign: a generation failed (see $out/campaign/failures)"
}
grep 'PATINA_CAMPAIGN_COVERAGE' <<<"$summary"
say "leg 4 passed: smoke campaign, every oracle in reach fired, every generation passed"

seeds=8
cargo patina explore run "$testbed" --seeds "$seeds" \
  --net-drop-permille 100 --net-latency-nanos 1000000 --net-jitter-nanos 0..2000000 \
  --net-connect-refuse-permille 100 --net-reset-permille 100 \
  >"$out/sweep.log" 2>&1 || {
  grep -E '^(PATINA_EXPLORE|MULTIPLAYER_)' "$out/sweep.log" >&2 || true
  fail "faulted sweep"
}
passed=$(grep -c '^MULTIPLAYER_RESULT ' "$out/sweep.log" || true)
resent=$(grep '^MULTIPLAYER_RESULT ' "$out/sweep.log" | grep -cv ' transport_retries=0 ' || true)
receipts=$(grep '^MULTIPLAYER_RESULT ' "$out/sweep.log" | grep -cv ' receipts=0 ' || true)
applied=$(grep -c '^PATINA_NET_FAULT_REPORT .* vacuous=0' "$out/sweep.log" || true)
[ "$passed" -eq "$seeds" ] || fail "faulted sweep: $passed of $seeds runs passed"
[ "$resent" -eq "$seeds" ] || fail "faulted sweep: only $resent of $seeds runs resent a request"
[ "$receipts" -gt 0 ] || fail "faulted sweep: no patch was answered from its receipt"
[ "$applied" -eq "$seeds" ] || fail "faulted sweep: a net fault knob was inert"
say "leg 5 passed: faulted sweep, $seeds seeds, every run resent, $receipts answered from a receipt"
