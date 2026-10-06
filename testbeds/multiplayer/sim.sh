#!/usr/bin/env bash
# The multiplayer testbed's legs of `mise run sim` (brief 6.1; PRACTICES, Simulation with
# patina). Not part of `mise run check`. Each leg fails loudly; none skips.
#
#   1. code: rustfmt, clippy with warnings denied, the unit tests;
#   2. build shim-linked; `cargo patina audit` finds exactly the unsupported symbols Turso
#      links (README, gap 4), which every run is then allowed by name;
#   3. determinism: one seed recorded twice is byte-identical, and its replay reports the
#      same verdict;
#   4. smoke campaign: buggify and PCT schedules over 16 generations, every `sometimes!`
#      and `reachable!` site fired but those out of reach below, and no failure but the
#      known product findings below;
#   5. faulted sweep: 8 seeds with dropped, refused, reset, and delayed connections; every
#      run passes, every run resent a request, some patch was answered from its receipt,
#      and no fault knob was inert;
#   6. known patina gaps: each must still reproduce exactly as recorded, so a patina bump
#      that fixes one gets its README section and DECISIONS.md entry updated.
#
# One current-thread runtime with the 1 ms ticker (the testbed's default) is the
# configuration the spike (1.3) found the shim carries.
set -euo pipefail
cd "$(dirname "$0")"

say() { printf 'sim: multiplayer: %s\n' "$*"; }
fail() {
  say "FAILED: $*" >&2
  exit 1
}
command -v cargo-patina >/dev/null || fail "cargo-patina is missing: run mise install"

# Product bugs this testbed found and DECISIONS.md records, by the label of the invariant a
# run breaks first. Until each is fixed, a campaign generation failing on one of these is
# reported, not fatal; any other failure is. When a fix lands, its label comes out of
# this list and its ignored test in crates/service/tests/in_flight.rs is un-ignored.
known_findings=()

# Coverage oracles in the binary that this testbed cannot reach, by label (DECISIONS.md,
# 6.1 integration). The campaign's own gate is waived for these alone: any other oracle that
# never fires fails leg 4, and one of these firing is reported so it can leave the list.
out_of_reach=(
  # 4.1's announce fallback: needs a journey patch with an entity create riding in it and
  # the store's revisions failing to read right after its commit; the testbed has neither.
  service-deployment-revision-unread
  # 4.1's receipt recheck after the engine answers stale: needs the original to commit
  # between a resubmission's receipt lookup and its load, and no site delays a load.
  service-duplicate-original-committed-meanwhile
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
)

# Builds the artifact first, so `target` (a managed symlink on some hosts) exists.
cargo fmt --check || fail "rustfmt: run cargo fmt in testbeds/multiplayer"
cargo clippy --locked --all-targets -- -D warnings || fail "clippy reported errors"
cargo test --locked || fail "unit tests failed"
say "leg 1 passed: code"

out=target/patina
mkdir -p "$out"
testbed=$out/cairn-multiplayer
cargo patina build . --output "$testbed" >"$out/build.log" 2>&1 || {
  cat "$out/build.log" >&2
  fail "cargo patina build"
}
# The audit exits nonzero on unsupported symbols; the leg checks they are exactly the
# recorded ones (gap 4) rather than none.
cargo patina audit "$testbed" >"$out/audit.log" 2>&1 || true
found=$(grep -oE '^    (instruction@[^ ]+ \(undecodable-instruction\) \[symbol=[a-z0-9_]+|(dlopen|dlclose) \(dynamic-loading\))' "$out/audit.log" |
  sed -E 's/.*\[symbol=//; s/^ *([a-z]+) \(dynamic-loading\)/\1/' | sort -u)
expected=$(sort -u unsupported-symbols.txt)
[ "$found" = "$expected" ] || {
  diff <(printf '%s\n' "$expected") <(printf '%s\n' "$found") >&2 || true
  fail "the audit's unsupported symbols changed: update unsupported-symbols.txt, README.md gap 4, and DECISIONS.md"
}
grep -q 'unsupported native imports' "$out/audit.log" || fail "the audit reports no unsupported symbols: gap 4 is fixed"
allow=$(paste -sd, unsupported-symbols.txt)
say "leg 2 passed: built; the audit finds exactly the $(wc -l <unsupported-symbols.txt) recorded unsupported symbols"

# The verdict line a run reported, from its stderr file.
verdict_of() { grep '^PATINA_VERDICT ' "$1" || true; }

for seed in 1 2; do
  for repeat in a b; do
    cargo patina run "$testbed" --seed "$seed" --record "$out/seed-$seed-$repeat.patina" \
      --allow-unsupported-symbols "$allow" >/dev/null 2>"$out/seed-$seed-$repeat.err" ||
      fail "seed $seed did not pass: $out/seed-$seed-$repeat.err"
  done
  cmp -s "$out/seed-$seed-a.patina" "$out/seed-$seed-b.patina" ||
    fail "seed $seed recorded twice gave different traces"
  cargo patina replay "$testbed" "$out/seed-$seed-a.patina" --allow-unsupported-symbols "$allow" \
    >/dev/null 2>"$out/seed-$seed-replay.err" || fail "seed $seed replay failed: $out/seed-$seed-replay.err"
  [ "$(verdict_of "$out/seed-$seed-a.err")" = "$(verdict_of "$out/seed-$seed-replay.err")" ] ||
    fail "seed $seed replay reported a different verdict"
done
say "leg 3 passed: repeats byte-identical, replays reproduce the verdict"

rm -rf "$out/campaign"
status=0
cargo patina campaign "$testbed" --gens 16 --buggify --sched-pct --out-dir "$out/campaign" \
  --progress-every 0 --allow-unmet-sometimes --allow-unsupported-symbols "$allow" \
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
known=0
while read -r class count; do
  [ "$class" = OK ] && continue
  [ "$class" = VIOLATION ] || fail "smoke campaign: $count generation(s) classed $class"
  known=$count
done < <(grep -E '^  class ' <<<"$summary" | awk '{ print $2, $3 }')
while read -r signature; do
  label=${signature##*label=}
  label=${label%%|*}
  printf '%s\n' "${known_findings[@]}" | grep -qx -- "$label" ||
    fail "smoke campaign: a new violation, $signature (see $out/campaign/failures)"
done < <(grep -oE 'signature: VIOLATION\|verdict kind=violation label=[a-z-]+\|' <<<"$summary" || true)
grep 'PATINA_CAMPAIGN_COVERAGE' <<<"$summary"
say "leg 4 passed: smoke campaign, every oracle in reach fired; $known generation(s) hit a known product finding"

seeds=8
cargo patina explore run "$testbed" --seeds "$seeds" \
  --net-drop-permille 100 --net-latency-nanos 1000000 --net-jitter-nanos 0..2000000 \
  --net-connect-refuse-permille 100 --net-reset-permille 100 \
  --allow-unsupported-symbols "$allow" >"$out/sweep.log" 2>&1 || {
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

# gap NAME PATTERN COMMAND...: COMMAND must fail and print PATTERN.
gap() {
  local name=$1 pattern=$2 status=0
  shift 2
  "$@" >"$out/gap-$name.log" 2>&1 || status=$?
  if [ "$status" -eq 0 ] || ! grep -Eq "$pattern" "$out/gap-$name.log"; then
    fail "patina gap '$name' no longer reproduces (exit $status): update testbeds/multiplayer/README.md and DECISIONS.md"
  fi
  say "gap $name still reproduces: $(grep -Eo "$pattern" "$out/gap-$name.log" | head -1)"
}
gap unsupported-symbols 'refusing to run .* neither interposed by the deterministic runtime nor known-safe' \
  cargo patina run "$testbed" --seed 1
# Built from inside its own workspace first, so `target` is the managed symlink there too.
(cd gaps && cargo build --locked --quiet) || fail "the gap reproducers do not build"
for reproducer in spaced_label always_abort; do
  cargo patina build gaps --bin "$reproducer" --output "$out/$reproducer" >"$out/$reproducer-build.log" 2>&1 || {
    cat "$out/$reproducer-build.log" >&2
    fail "cargo patina build gaps --bin $reproducer"
  }
done
rm -rf "$out/spaced-campaign" "$out/always-abort.patina"
gap spaced-label 'malformed SDK declared-site token' \
  cargo patina campaign "$out/spaced_label" --gens 1 --out-dir "$out/spaced-campaign" --progress-every 0
gap always-abort 'record finalization did not complete' \
  cargo patina run "$out/always_abort" --seed 1 --record "$out/always-abort.patina"
say "leg 6 passed: known gaps reproduce as recorded"
