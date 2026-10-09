# shellcheck shell=bash
# Sourced by every rung of the validation ladder (PRACTICES, The validation ladder).
#
# A rung reports how many things each of its suites checked or ran, through
# `ladder_suite`, and fails when any suite reports none, so one suite vanishing cannot hide
# behind another's count. A rung that reports no suite at all fails too (`ladder_rung_end`).
# A missing tool fails the rung rather than skipping it (`ladder_require_tool`).
#
# The caller sets LADDER_RUNG (its number) before sourcing this file. Every line meant for
# the ladder's summary starts with "ladder: rung N:", which scripts/ladder collects.

: "${LADDER_RUNG:?set LADDER_RUNG before sourcing scripts/ladder-lib.sh}"
ladder_suite_count=0

ladder_say() {
  printf 'ladder: rung %s: %s\n' "$LADDER_RUNG" "$*"
}

ladder_fail() {
  ladder_say "FAILED: $*" >&2
  exit 1
}

# ladder_fixing: true when the rung may change files to apply what its tools can fix itself.
# A run with CI unset (a developer's machine) does; CI, even set empty, only verifies, so a change that was not
# formatted or fixed fails there instead of being repaired in a throwaway checkout.
ladder_fixing() {
  [ -z "${CI+set}" ]
}

# ladder_source_sums: one "checksum size path" line per source file a fixer may rewrite (the
# Rust crates and testbeds, and the web packages), so two runs of it differ by the files that
# changed in between.
ladder_source_sums() {
  find crates testbeds web \( -name node_modules -o -name target -o -name dist \) -prune -o \
    -type f \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.css' \) -print0 |
    xargs -0 cksum | sort -k3
}

# ladder_changed_files BEFORE AFTER: how many files differ between two ladder_source_sums
# outputs (each given as a string).
ladder_changed_files() {
  diff <(printf '%s\n' "$1") <(printf '%s\n' "$2") | grep -c '^>' || true
}

# ladder_require_tool NAME COMMAND...: fail the rung unless COMMAND (a version query) runs.
ladder_require_tool() {
  local name=$1
  shift
  "$@" >/dev/null 2>&1 || ladder_fail "tool missing: $name ('$*' did not run)"
}

# ladder_suite NAME COUNT UNIT: record that suite NAME checked or ran COUNT UNITs.
ladder_suite() {
  local name=$1 count=$2 unit=$3
  ladder_suite_count=$((ladder_suite_count + 1))
  [ "$count" -gt 0 ] || ladder_fail "suite $name checked or ran 0 $unit"
  ladder_say "suite $name: $count $unit"
}

# ladder_rung_end: call last; a rung that reported no suite checked nothing.
ladder_rung_end() {
  [ "$ladder_suite_count" -gt 0 ] || ladder_fail "reported no suites"
  ladder_say "passed"
}

# The tests rung 2 leaves to later rungs, by name (the module paths PRACTICES lists): rung 2
# and `mise run test` without a filter run everything else.
ladder_unit_skips=(--skip property:: --skip conformance:: --skip cost:: --skip in_process:: --skip binary::binary::)

# ladder_cargo_test [--test NAME] [-- TEST_ARGUMENT...]: the workspace's tests (every crate,
# every feature), TEST_ARGUMENT being the name filters. Prints the run's output and sets
# `ladder_tests` to one "package<TAB>test<TAB>result" line per test it ran. Every rung runs
# its Rust tests in this one shape: scripts/ladder-tests builds the workspace's test binaries
# in one pass (a no-op after the first rung's) and runs them in parallel, each rung picking
# the ones it owns by name. Cargo resolves features per invocation, so a run scoped to one
# crate (`-p`) would compile its own copy of every dependency whose features differ and link
# its own test binaries. Returns the run's status.
ladder_cargo_test() {
  local status=0 results=dist/ladder/rung-$LADDER_RUNG-tests
  mkdir -p dist/ladder
  scripts/ladder-tests "$results" "$@" || status=$?
  ladder_tests=$(cat "$results" 2>/dev/null || true)
  return "$status"
}

# ladder_tests_matching PACKAGE REGEX: how many of the last ladder_cargo_test run's tests
# belong to PACKAGE and have a name matching REGEX (an awk extended regular expression).
ladder_tests_matching() {
  awk -F'\t' -v package="$1" -v pattern="$2" '$1 == package && $2 ~ pattern { count++ } END { print count + 0 }' <<<"$ladder_tests"
}
