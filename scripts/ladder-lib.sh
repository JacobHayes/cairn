# shellcheck shell=bash
# Sourced by every rung of the validation ladder (PRACTICES, The validation ladder).
#
# A rung reports how many things each of its suites checked or ran, through
# `ladder_suite`, and fails when any suite reports none, so one suite vanishing cannot hide
# behind another's count. A rung that reports no suite at all fails too (`ladder_rung_end`).
# A missing tool fails the rung rather than skipping it (`ladder_require_tool`).
#
# Quiet by design: everything a rung runs writes to one log (dist/ladder/last.log, which
# scripts/ladder truncates once per run), and the terminal gets only what a person acts on
# (`ladder_show`): one line when the rung passes, and on a failure the errors, the failing
# tests with their output, the failing lint lines, and how to rerun. Descriptor 3 is the
# terminal; a command that starts something long-lived closes it (`3>&-`).
#
# The caller sets LADDER_RUNG (its number) before sourcing this file; LADDER_NAME names it in
# the lines it prints (default "rung N").

: "${LADDER_RUNG:?set LADDER_RUNG before sourcing scripts/ladder-lib.sh}"
ladder_name=${LADDER_NAME:-rung $LADDER_RUNG}
ladder_suite_count=0
ladder_test_count=0
ladder_started=${LADDER_STARTED:-$(date +%s)}
ladder_reported=
# A rung run alone starts its own log and says where it is; under scripts/ladder, the
# ladder's last line does.
ladder_alone=
if [ -z "${LADDER_LOG:-}" ]; then
  ladder_alone=1
  LADDER_LOG=dist/ladder/last.log
  mkdir -p "$(dirname "$LADDER_LOG")"
  : >"$LADDER_LOG"
fi
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
  [ -z "$ladder_alone" ] || ladder_show "log: $LADDER_LOG"
  exit 1
}

# ladder_exit STATUS: the rung's exit trap. A command that failed under `set -e` outside a
# check (cargo metadata on a malformed manifest, say) leaves nothing on the terminal, so the
# end of the log stands in for the diagnostic.
ladder_exit() {
  [ "$1" -eq 0 ] || [ -n "$ladder_reported" ] || {
    ladder_show "$ladder_name: FAILED in $(ladder_elapsed): exit status $1; the end of the log:"
    tail -n 20 "$LADDER_LOG" | ladder_show
    [ -z "$ladder_alone" ] || ladder_show "log: $LADDER_LOG"
  }
}
trap 'ladder_exit $?' EXIT

# ladder_run WHAT COMMAND...: run COMMAND with its output in the log; when it fails, show the
# output and fail the rung. Give cargo -q, so its output is only diagnostics.
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
  [ "$unit" != tests ] || ladder_test_count=$((ladder_test_count + count))
  ladder_say "suite $name: $count $unit"
}

# ladder_rung_end: call last; a rung that reported no suite checked nothing.
ladder_rung_end() {
  [ "$ladder_suite_count" -gt 0 ] || ladder_fail "reported no suites"
  # What a passing rung says: one line, with how much it ran (tests, or suites for a rung of tools).
  ladder_show "$ladder_name: passed, $([ "$ladder_test_count" -gt 0 ] && echo "$ladder_test_count tests" || echo "$ladder_suite_count checks"), $(ladder_elapsed)"
  [ -z "$ladder_alone" ] || ladder_show "log: $LADDER_LOG"
}

# The tests rung 2 leaves to later rungs, by name (the module paths PRACTICES lists): rung 2
# runs everything else.
ladder_unit_skips=(--skip property:: --skip conformance:: --skip cost:: --skip in_process:: --skip binary::binary::)

# ladder_cargo_test [--test NAME] [-- TEST_ARGUMENT...]: the workspace's tests (every crate,
# every feature), TEST_ARGUMENT being the name filters. Logs the run's output, shows the
# failing tests, and sets `ladder_tests` to one "package<TAB>test<TAB>result" line per test it
# ran. Every rung runs
# its Rust tests in this one shape: scripts/ladder-tests builds the workspace's test binaries
# in one pass (a no-op after the first rung's) and runs them in parallel, each rung picking
# the ones it owns by name. Cargo resolves features per invocation, so a run scoped to one
# crate (`-p`) would compile its own copy of every dependency whose features differ and link
# its own test binaries. Returns the run's status.
ladder_cargo_test() {
  local status=0 results=dist/ladder/rung-$LADDER_RUNG-tests
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
  local kind=$1 suites=0 crate directory file count expected members
  # Workspace members as "name<TAB>directory" lines, relative to the repository root.
  # shellcheck disable=SC2016 # the backticks are a JavaScript template literal
  members=$(cargo metadata --no-deps --format-version 1 --locked | node -e '
    const path = require("path");
    let text = "";
    process.stdin.on("data", (chunk) => (text += chunk)).on("end", () => {
      for (const item of JSON.parse(text).packages)
        console.log(`${item.name}\t${path.relative(process.cwd(), path.dirname(item.manifest_path))}`);
    });')
  while IFS=$'\t' read -r crate directory; do
    [ -n "$crate" ] || continue
    while IFS=$'\t' read -r file count; do
      [ -n "$file" ] || continue
      ladder_suite "$directory/tests/integration/$file.rs" "$count" tests
      suites=$((suites + 1))
    done < <(awk -F'\t' -v crate="$crate" -v kind="$kind" '
      $1 == crate && match($2, "^[a-z0-9_]+::" kind "::") {
        split($2, path, "::"); count[path[1]]++ }
      END { for (file in count) print file "\t" count[file] }' <<<"$ladder_tests" | sort)
  done <<<"$members"
  expected=$(find crates -path "*/tests/integration/${kind}_*.rs" | wc -l)
  [ "$suites" -eq "$expected" ] || ladder_fail "ran $suites $kind test files; the tree has $expected"
}
