# shellcheck shell=bash
# Sourced by every step of the validation ladder (PRACTICES, The validation ladder).
#
# A step reports how many things each of its suites checked or ran, through
# `ladder_suite`, and fails when any suite reports none, so one suite vanishing cannot hide
# behind another's count. A step that reports no suite at all fails too (`ladder_step_end`).
# A missing tool fails the step rather than skipping it (`ladder_require_tool`).
#
# Quiet by design: everything a step runs writes to its own log (dist/ladder/<step>.log), and
# the terminal gets only what a person acts on (`ladder_show`): one line when the step
# passes, and on a failure the errors, the failing tests with their output, the failing lint
# lines, how to rerun, and the log's path. Descriptor 3 is the terminal; a command that
# starts something long-lived closes it (`3>&-`).
#
# The caller sets LADDER_STEP (its name, which names its log and prefixes the lines it
# prints) before sourcing this file.

: "${LADDER_STEP:?set LADDER_STEP before sourcing scripts/ladder-lib.sh}"
ladder_name=$LADDER_STEP
ladder_suite_count=0
ladder_test_count=0
ladder_started=${LADDER_STARTED:-$(date +%s)}
ladder_reported=
LADDER_LOG=dist/ladder/$LADDER_STEP.log
mkdir -p dist/ladder
: >"$LADDER_LOG"
exec 3>&1 >>"$LADDER_LOG" 2>&1
printf '== %s ==\n' "$ladder_name"

# ladder_show [LINE...]: print to the terminal, and to the log, the LINEs, or stdin when there
# are none. Only for what a person acts on.
ladder_show() {
  local text
  if [ $# -gt 0 ]; then text=$(printf '%s\n' "$@"); else text=$(cat); fi
  [ -z "$text" ] || { printf '%s\n' "$text"; printf '%s\n' "$text" >&3; }
}

ladder_say() {
  printf '%s: %s\n' "$ladder_name" "$*"
}

ladder_elapsed() {
  echo "$(($(date +%s) - ladder_started))s"
}

ladder_fail() {
  ladder_reported=1
  ladder_show "$ladder_name: FAILED in $(ladder_elapsed): $*"
  ladder_show "log: $LADDER_LOG"
  exit 1
}

# ladder_exit STATUS: the step's exit trap. A command that failed under `set -e` outside a
# check (cargo metadata on a malformed manifest, say) leaves nothing on the terminal, so the
# end of the log stands in for the diagnostic.
ladder_exit() {
  [ "$1" -eq 0 ] || [ -n "$ladder_reported" ] || {
    ladder_show "$ladder_name: FAILED in $(ladder_elapsed): exit status $1; the end of the log:"
    tail -n 20 "$LADDER_LOG" | ladder_show
    ladder_show "log: $LADDER_LOG"
  }
}
trap 'ladder_exit $?' EXIT

# ladder_run WHAT COMMAND...: run COMMAND with its output in the log; when it fails, show the
# output and fail the step. Give cargo -q, so its output is only diagnostics.
ladder_run() {
  local what=$1 output status=0
  shift
  output=$("$@" 2>&1) || status=$?
  printf '%s\n' "$output"
  if [ "$status" -ne 0 ]; then
    ladder_show "$output"
    ladder_fail "$what failed"
  fi
}

# ladder_fixing: true when the step may change files to apply what its tools can fix itself.
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

# ladder_require_tool NAME COMMAND...: fail the step unless COMMAND (a version query) runs.
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
  [ "$unit" != tests ] || ladder_test_count=$((ladder_test_count + count))
  ladder_say "suite $name: $count $unit"
}

# ladder_step_end: call last; a step that reported no suite checked nothing.
ladder_step_end() {
  [ "$ladder_suite_count" -gt 0 ] || ladder_fail "reported no suites"
  # What a passing step says: one line, with how much it ran (tests, or suites for a step of tools).
  ladder_show "$ladder_name: passed, $([ "$ladder_test_count" -gt 0 ] && echo "$ladder_test_count tests" || echo "$ladder_suite_count checks"), $(ladder_elapsed)"
}

# The tests the unit step leaves to later steps, by name (the module paths PRACTICES lists):
# it runs everything else.
ladder_unit_skips=(--skip property:: --skip conformance:: --skip cost:: --skip in_process:: --skip binary::binary::)

# ladder_cargo_test [--test NAME] [-- TEST_ARGUMENT...]: the workspace's tests (every crate,
# every feature), TEST_ARGUMENT being the name filters. Logs the run's output, shows the
# failing tests, and sets `ladder_tests` to one "package<TAB>test<TAB>result" line per test it
# ran. Every step runs
# its Rust tests in this one shape: scripts/ladder-tests builds the workspace's test binaries
# in one pass (scripts/ladder does it once, before the first step that needs them) and runs
# them in parallel, each step picking the ones it owns by name. Cargo resolves features per invocation, so a run scoped to one
# crate (`-p`) would compile its own copy of every dependency whose features differ and link
# its own test binaries. Returns the run's status.
ladder_cargo_test() {
  local status=0 results=dist/ladder/$LADDER_STEP-tests
  mkdir -p dist/ladder
  scripts/ladder-tests "$results" "$@" 2>"$results.err" || status=$?
  ladder_show <"$results.err"
  ladder_tests=$(cat "$results" 2>/dev/null || true)
  return "$status"
}

# ladder_tests_matching PACKAGE REGEX: how many of the last ladder_cargo_test run's tests
# belong to PACKAGE and have a name matching REGEX (an awk extended regular expression).
ladder_tests_matching() {
  awk -F'\t' -v package="$1" -v pattern="$2" '$1 == package && $2 ~ pattern { count++ } END { print count + 0 }' <<<"$ladder_tests"
}

# ladder_file_suites KIND: one suite per test file of KIND ("property" or "cost") of each
# crate, counted from the last ladder_cargo_test run's test names (`<file>::KIND::<test>`),
# and a failure when the tree has a KIND test file that is not a suite, so one that stops
# building or is not a module of its crate's test binary does not go unnoticed.
ladder_file_suites() {
  local kind=$1 suites=0 crate file count expected
  while IFS= read -r crate; do
    [ -n "$crate" ] || continue
    while IFS=$'\t' read -r file count; do
      [ -n "$file" ] || continue
      ladder_suite "$crate $file" "$count" tests
      suites=$((suites + 1))
    done < <(awk -F'\t' -v crate="$crate" -v kind="$kind" '
      $1 == crate && match($2, "^[a-z0-9_]+::" kind "::") {
        split($2, path, "::"); count[path[1]]++ }
      END { for (file in count) print file "\t" count[file] }' <<<"$ladder_tests" | sort)
  done < <(cargo metadata --no-deps --format-version 1 --locked | node -e '
    let text = "";
    process.stdin.on("data", (chunk) => (text += chunk)).on("end", () => {
      for (const item of JSON.parse(text).packages) console.log(item.name);
    });')
  expected=$(find crates -path '*/tests/integration/*' -name "${kind}_*.rs" | wc -l)
  [ "$suites" -eq "$expected" ] || ladder_fail "ran $suites $kind test files; the tree has $expected"
}
